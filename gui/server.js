/**
 * BlitzArch GUI Backend Server
 * Simple Express server to bridge web frontend with Rust CLI
 */

import express from 'express';
import { spawn } from 'child_process';
import path from 'path';
import fs from 'fs/promises';
import { fileURLToPath } from 'url';
import cors from 'cors';
import multer from 'multer';
import os from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const app = express();
const PORT = process.env.PORT || 3001;

// Configure multer for file uploads
const upload = multer({
  dest: path.join(os.tmpdir(), 'blitzarch-uploads'),
  limits: {
    fileSize: 100 * 1024 * 1024, // 100MB limit
    files: 50 // Max 50 files
  }
});

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.static('dist'));

// Path to BlitzArch executable
const BLITZARCH_PATH = path.join(__dirname, '../target/release/blitzarch');

/**
 * Execute BlitzArch CLI command
 */
async function executeBlitzArch(args) {
  return new Promise((resolve) => {
    console.log('Executing BlitzArch:', BLITZARCH_PATH, args);
    
    const process = spawn(BLITZARCH_PATH, args, {
      stdio: ['pipe', 'pipe', 'pipe']
    });

    let stdout = '';
    let stderr = '';

    process.stdout.on('data', (data) => {
      stdout += data.toString();
    });

    process.stderr.on('data', (data) => {
      stderr += data.toString();
    });

    process.on('close', (code) => {
      resolve({
        success: code === 0,
        output: stdout,
        error: stderr,
        exitCode: code
      });
    });

    process.on('error', (error) => {
      resolve({
        success: false,
        output: '',
        error: error.message,
        exitCode: -1
      });
    });
  });
}

// API Routes

/**
 * Upload files and create archive endpoint
 */
app.post('/api/upload-and-create-archive', upload.array('files'), async (req, res) => {
  console.log('🔍 Upload request received');
  console.log('📋 Request body:', req.body);
  console.log('📁 Request files:', req.files ? req.files.length : 'undefined');
  console.log('📄 Files details:', req.files ? req.files.map(f => ({ name: f.originalname, size: f.size })) : 'no files');
  
  try {
    const { archiveName, compressionLevel = 3, password, bundleSize = 32, outputDirectory } = req.body;
    
    if (!req.files || req.files.length === 0) {
      console.log('❌ No files found in request');
      return res.status(400).json({
        success: false,
        error: 'No files uploaded'
      });
    }
    
    console.log(`📁 Uploaded ${req.files.length} files for archive creation`);
    console.log('📋 Files:', req.files.map(f => f.originalname));
    
    // Get uploaded file paths
    const filePaths = req.files.map(file => file.path);
    
    // Create archive name if not provided
    const finalArchiveName = archiveName || `archive-${Date.now()}`;
    
    // Use provided output directory or fallback to Downloads
    let targetDirectory;
    if (outputDirectory && outputDirectory !== 'undefined') {
      targetDirectory = outputDirectory;
      console.log('🎯 Using provided directory:', targetDirectory);
    } else {
      targetDirectory = path.join(os.homedir(), 'Downloads');
      console.log('📁 Fallback to Downloads:', targetDirectory);
    }
    
    const outputPath = path.join(targetDirectory, `${finalArchiveName}.blz`);
    
    // Build BlitzArch command
    const args = ['create'];
    args.push('--output', outputPath);
    args.push('--bundle-size', bundleSize.toString());
    
    if (compressionLevel !== 3) {
      args.push('--level', compressionLevel.toString());
    }
    
    if (password) {
      args.push('--password', password);
    }
    
    // Add input files
    filePaths.forEach(filePath => {
      args.push(filePath);
    });
    
    console.log('🚀 Executing BlitzArch with args:', args);
    
    // Execute BlitzArch
    const result = await executeBlitzArch(args);
    
    // Clean up uploaded files
    try {
      await Promise.all(filePaths.map(filePath => fs.unlink(filePath)));
    } catch (cleanupError) {
      console.warn('⚠️ Failed to clean up some uploaded files:', cleanupError.message);
    }
    
    if (result.success) {
      res.json({
        success: true,
        output: result.output,
        archivePath: outputPath,
        archiveName: finalArchiveName,
        downloadUrl: `/api/download-archive/${encodeURIComponent(path.basename(outputPath))}`
      });
    } else {
      res.json({
        success: false,
        error: result.error
      });
    }
    
  } catch (error) {
    console.error('❌ Error in upload-and-create-archive:', error);
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

/**
 * Main BlitzArch command endpoint
 */
app.post('/api/blitzarch', async (req, res) => {
  try {
    const { args } = req.body;
    
    if (!Array.isArray(args)) {
      return res.status(400).json({
        success: false,
        error: 'Invalid arguments format'
      });
    }

    const result = await executeBlitzArch(args);
    res.json(result);
  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

/**
 * Delete file endpoint
 */
app.post('/api/delete-file', async (req, res) => {
  try {
    const { filePath } = req.body;
    
    if (!filePath) {
      return res.status(400).json({
        success: false,
        error: 'File path is required'
      });
    }

    // Security check - only allow deletion of .blz files in safe directories
    if (!filePath.endsWith('.blz') && !filePath.endsWith('.katana')) {
      return res.status(400).json({
        success: false,
        error: 'Only archive files can be deleted'
      });
    }

    await fs.unlink(filePath);
    res.json({ success: true });
  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

/**
 * Get BlitzArch version and status
 */
app.get('/api/status', async (req, res) => {
  try {
    const result = await executeBlitzArch(['--version']);
    res.json({
      available: result.success,
      version: result.success ? result.output.trim() : null,
      error: result.error || null
    });
  } catch (error) {
    res.json({
      available: false,
      version: null,
      error: error.message
    });
  }
});

/**
 * List files in directory
 */
app.post('/api/list-directory', async (req, res) => {
  try {
    const { dirPath } = req.body;
    
    if (!dirPath) {
      return res.status(400).json({
        success: false,
        error: 'Directory path is required'
      });
    }

    const files = await fs.readdir(dirPath, { withFileTypes: true });
    const fileList = files.map(file => ({
      name: file.name,
      isDirectory: file.isDirectory(),
      path: path.join(dirPath, file.name)
    }));

    res.json({
      success: true,
      files: fileList
    });
  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

/**
 * Get file info
 */
app.post('/api/file-info', async (req, res) => {
  try {
    const { filePath } = req.body;
    
    if (!filePath) {
      return res.status(400).json({
        success: false,
        error: 'File path is required'
      });
    }

    const stats = await fs.stat(filePath);
    res.json({
      success: true,
      info: {
        size: stats.size,
        modified: stats.mtime,
        created: stats.birthtime,
        isDirectory: stats.isDirectory(),
        isFile: stats.isFile()
      }
    });
  } catch (error) {
    res.status(500).json({
      success: false,
      error: error.message
    });
  }
});

/**
 * Validate directory for writing
 */
app.post('/api/validate-directory', async (req, res) => {
  try {
    const { dirPath } = req.body;
    
    if (!dirPath) {
      return res.status(400).json({
        valid: false,
        error: 'Directory path is required'
      });
    }

    // Check if directory exists
    try {
      const stats = await fs.stat(dirPath);
      if (!stats.isDirectory()) {
        return res.json({
          valid: false,
          error: 'Path is not a directory'
        });
      }
    } catch (error) {
      return res.json({
        valid: false,
        error: 'Directory does not exist'
      });
    }

    // Check write permissions by trying to create a temporary file
    const tempFile = path.join(dirPath, '.blitzarch-test-' + Date.now());
    try {
      await fs.writeFile(tempFile, 'test');
      await fs.unlink(tempFile);
      
      res.json({
        valid: true,
        writable: true
      });
    } catch (error) {
      res.json({
        valid: true,
        writable: false,
        error: 'Directory is not writable'
      });
    }
  } catch (error) {
    res.status(500).json({
      valid: false,
      error: error.message
    });
  }
});

// Serve frontend for all other routes
app.get('*', (req, res) => {
  res.sendFile(path.join(__dirname, 'dist', 'index.html'));
});

// Error handling middleware
app.use((error, req, res, next) => {
  console.error('Server error:', error);
  res.status(500).json({
    success: false,
    error: 'Internal server error'
  });
});

// Start server
app.listen(PORT, () => {
  console.log(`🚀 BlitzArch GUI Server running on http://localhost:${PORT}`);
  console.log(`📁 BlitzArch executable: ${BLITZARCH_PATH}`);
  
  // Check if BlitzArch executable exists
  fs.access(BLITZARCH_PATH)
    .then(() => console.log('✅ BlitzArch executable found'))
    .catch(() => console.log('❌ BlitzArch executable not found - please build the project first'));
});

export default app;
