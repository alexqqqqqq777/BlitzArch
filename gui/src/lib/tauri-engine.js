/**
 * Tauri BlitzArch Engine
 * Direct integration with Rust backend via Tauri commands
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

class TauriBlitzArchEngine {
  constructor() {
    this.isProcessing = false;
    this.currentProcess = null;
  }

  /**
   * Create archive using Tauri command (direct Rust call)
   */
  async createArchive(files, archiveName, outputDir, options = {}) {
    const {
      compressionLevel = 3,
      password = null,
      bundleSize = 32,
      threads = null,
      codecThreads = 0,
      memoryBudget = 0
    } = options;

    console.log('🎯 Tauri Archive Creation:');
    console.log('📦 Archive Name:', archiveName);
    console.log('📁 Output Directory:', outputDir);
    console.log('📋 Input Files:', files);

    this.isProcessing = true;

    try {
      // Call Tauri async command for non-blocking operation
      const outputPath = `${outputDir}/${archiveName}.blz`;
      const result = await invoke('create_archive_async', {
        inputs: files,
        outputPath: outputPath,
        output_path: outputPath,
        compressionLevel: compressionLevel,
        compression_level: compressionLevel,
        bundleSize: bundleSize,
        bundle_size: bundleSize,
        password: password,
        threads: threads, // null -> авто выбор потока
        codecThreads: codecThreads,
        codec_threads: codecThreads,
        memoryBudget: memoryBudget,
        memory_budget: memoryBudget
      });

      console.log('✅ Tauri command result:', result);

      if (result.success) {
        return {
          success: true,
          output: result.output,
          archivePath: result.archive_path
        };
      } else {
        return {
          success: false,
          error: result.error
        };
      }
    } catch (error) {
      console.error('💥 Tauri command failed:', error);
      return {
        success: false,
        error: error.toString()
      };
    } finally {
      this.isProcessing = false;
    }
  }

  /**
   * Get parent directory of a file using Tauri
   */
  async getParentDirectory(filePath) {
    try {
      const parentDir = await invoke('get_parent_directory', {
        filePath: filePath
      });
      return parentDir;
    } catch (error) {
      console.error('❌ Failed to get parent directory:', error);
      return null;
    }
  }

  /**
   * Calculate optimal strip_components based on archive contents
   */
  async calculateStripComponents(archivePath) {
    try {
      // Get archive contents first
      const result = await this.listArchive(archivePath);
      
      if (!result || !result.success || !result.files || result.files.length === 0) {
        return 0;
      }

      // Find common root path
      let recursiveFixCount = 0;

      const files = result.files;
    console.log('🔍 calculateStripComponents: got', files.length, 'files');
    console.log('🔍 First 3 files:', files.slice(0, 3));
    
    const paths = files.map(file => file.path || file.name || file);
    console.log('🔍 First 3 paths:', paths.slice(0, 3));
      
      // Handle single file case
      if (paths.length === 1) {
        const components = paths[0].split('/').filter(c => c.length > 0);
        return Math.max(0, components.length - 1); // Keep filename
      }

      // Find longest common prefix for multiple files
      let commonPrefix = paths[0] || '';
      
      for (let i = 1; i < paths.length; i++) {
        const path = paths[i] || '';
        let j = 0;
        while (j < Math.min(commonPrefix.length, path.length) && 
               commonPrefix[j] === path[j]) {
          j++;
        }
        commonPrefix = commonPrefix.substring(0, j);
        
        if (!commonPrefix) break;
      }

      // Convert common prefix to component count
      if (!commonPrefix) {
        return 0; // No common path
      }

      // Count directory components in common prefix
      // Remove trailing filename if present
      const lastSlash = commonPrefix.lastIndexOf('/');
      const commonDir = lastSlash >= 0 ? commonPrefix.substring(0, lastSlash) : '';
      
      if (!commonDir) {
        return 0;
      }

      const components = commonDir.split('/').filter(c => c.length > 0);
      
      // Skip common system paths like /Users/username
      let stripCount = components.length;
      
      // Heuristic: if path starts with /Users/ or similar, strip more aggressively
      if (commonDir.startsWith('/Users/') || commonDir.startsWith('/home/')) {
        stripCount = Math.max(stripCount - 1, 0);
      }
      
      console.log(`📊 Auto-calculated strip_components: ${stripCount} (common prefix: "${commonDir}")`);
    console.log(`🔍 Debug: commonPrefix="${commonPrefix}", commonDir="${commonDir}", components=`, components);
      return stripCount;
      
    } catch (error) {
      console.warn('⚠️ Failed to calculate strip_components:', error);
      return 0; // Safe fallback
    }
  }

  /**
   * Extract archive using Tauri with smart strip_components
   */
  async extractArchive(archivePath, outputDir, options = {}) {
    try {
      console.log('🔄 Extracting archive via Tauri:', archivePath, 'to', outputDir);
      
      // Auto-calculate strip_components if not provided
      let stripComponents = options.stripComponents;
      
      console.log('🔍 Extract conditions debug:');
      console.log('  - options.stripComponents:', options.stripComponents);
      console.log('  - options.autoStripComponents:', options.autoStripComponents);
      console.log('  - options.specificFiles:', options.specificFiles);
      console.log('  - stripComponents (initial):', stripComponents);

    // При выборочном извлечении (specificFiles) запрещаем auto-strip и принудительно ставим 0,
    // иначе движок не найдёт совпадений по путям и извлечёт всё содержимое.
    if (options.specificFiles && options.specificFiles.length > 0) {
      console.log('🎯 Using specificFiles mode: stripComponents = 0');
      stripComponents = 0;
    } else if (stripComponents === undefined && options.autoStripComponents !== false) {
      console.log('🤖 Calling calculateStripComponents...');
      stripComponents = await this.calculateStripComponents(archivePath);
      console.log(`🧠 Using auto-calculated strip_components: ${stripComponents}`);
    } else {
      console.log('🔧 Conditions not met for auto-calculation:');
      console.log('  - stripComponents === undefined:', stripComponents === undefined);
      console.log('  - options.autoStripComponents !== false:', options.autoStripComponents !== false);
    }  
      
      const result = await invoke('extract_archive_async', {
        // dual naming for compatibility
        archive_path: archivePath,
        archivePath: archivePath,
        output_dir: outputDir,
        outputDir: outputDir,
        password: options.password || null,
        strip_components: stripComponents != null ? stripComponents : null,
        stripComponents: stripComponents != null ? stripComponents : null,
        specific_files: options.specificFiles || null,
        specificFiles: options.specificFiles || null
      });
      
      if (result.success) {
        console.log('✅ Archive extracted successfully');
        return {
          success: true,
          output: result.output
        };
      } else {
        console.error('❌ Archive extraction failed:', result.error);
        return {
          success: false,
          error: result.error
        };
      }
    } catch (error) {
      console.error('💥 Tauri extract command failed:', error);
      return {
        success: false,
        error: error.toString()
      };
    }
  }

  /**
   * List archive contents using Tauri (async native index)
   */
  async listArchive(archivePath) {
    try {
      console.log('📋 Listing archive via Tauri (native index):', archivePath);
      
      const entries = await invoke('list_archive_async', {
        archivePath: archivePath
      });

      // entries is an array of objects { path, size, is_dir }
      if (Array.isArray(entries)) {
        const files = entries.filter(e => !e.is_dir).map(e => ({
          name: e.path.split('/').pop(),
          path: e.path,
          size: e.size || 0,
          crc32: '',
          crc_ok: true,
          is_dir: false
        }));
        return { success: true, files };
      }

      // Fall back to old CLI approach if needed
      const result = await invoke('list_archive', {
        archivePath: archivePath
      });
      
      if (result.success) {
        console.log('✅ Archive listed successfully');
        console.log('🔍 Output length:', result.output ? result.output.length : 0);
        
        // For large archives, only show truncated output to avoid browser crash
        if (result.output && result.output.length > 5000) {
          console.log('🔍 Raw CLI output (first 1000 chars):', result.output.substring(0, 1000));
          console.log('🔍 Raw CLI output (last 1000 chars):', result.output.substring(result.output.length - 1000));
        } else {
          console.log('🔍 Raw CLI output:', result.output);
        }
        
        // Declare variables outside try-block for proper scope
        let lines = [];
        let fileLines = [];
        
        try {
          console.log('🔍 Starting line parsing...');
          
          // Parse the output to extract file metadata
          lines = result.output ? result.output.split('\n').filter(line => line.trim()) : [];
          console.log('🔍 Parsed', lines.length, 'lines');
          
          fileLines = lines.filter(line => line.startsWith('- '));
          console.log('🔍 Found', fileLines.length, 'file lines');
          
          // For debugging, show first few file lines
          console.log('🔍 First 3 file lines:', fileLines.slice(0, 3));
          
        } catch (parseError) {
          console.error('💥 Error during line parsing:', parseError);
          throw parseError;
        }
        
        // Helper function to parse size strings like "10 bytes", "1.7 MB", "256 KB", etc.
        function parseSizeString(sizeStr) {
          // Handle "bytes" vs "B" suffix
          const normalizedStr = sizeStr.replace(/\bbytes?\b/i, 'B');
          const match = normalizedStr.match(/([\d.]+)\s*(B|KB|MB|GB)/i);
          if (!match) return 0;
          
          const value = parseFloat(match[1]);
          const unit = match[2].toUpperCase();
          
          switch (unit) {
            case 'GB': return Math.round(value * 1024 * 1024 * 1024);
            case 'MB': return Math.round(value * 1024 * 1024);
            case 'KB': return Math.round(value * 1024);
            case 'B': return Math.round(value);
            default: return 0;
          }
        }
        
        console.log('🔍 Starting file object creation...');
        // Counter for sanitized recursive objects across all files
        let recursiveFixCount = 0;
        
        // Parse lines like: "- tmp/test_file.txt (10 bytes)", etc.
        let processingCount = 0;
        const files = fileLines
          
          .map((line, index) => {
            try {
              processingCount++; // silent count
              
              const withoutPrefix = line.substring(2); // Remove "- "
              
              // Extract path (everything before first parenthesis)
              const pathMatch = withoutPrefix.match(/^(.+?)\s*\(/);
              const path = pathMatch ? pathMatch[1].trim() : withoutPrefix.trim();
              
              if (!path || path.length === 0) {
                console.warn(`⚠️ Empty path for line ${index + 1}:`, line);
                return null;
              }
              
              // Extract size from parentheses
              const sizeMatch = withoutPrefix.match(/\(([^)]+)\)/);
              const sizeStr = sizeMatch ? sizeMatch[1] : '0 bytes';
              const size = parseSizeString(sizeStr);
              
              // Extract filename from path
              const name = path.split('/').pop() || path;
              
              const fileObj = {
                name: name,
                path: path,
                size: size,
                crc32: '', // No CRC in basic list output
                crc_ok: true, // Assume OK since no validation info available
                is_dir: false // CLI list doesn't distinguish directories from files
              };
              
              // INTENSIVE DIAGNOSTICS for first few files
              if (index < 5) {
                console.log(`🔍 DETAILED FILE ${index + 1} DIAGNOSTICS:`);
                console.log(`  - Original path string: "${path}" (type: ${typeof path})`);
                console.log(`  - Original name string: "${name}" (type: ${typeof name})`);
                console.log(`  - fileObj.path: "${fileObj.path}" (type: ${typeof fileObj.path})`);
                console.log(`  - fileObj.name: "${fileObj.name}" (type: ${typeof fileObj.name})`);
                console.log(`  - Full fileObj:`, JSON.stringify(fileObj, null, 2));
              }
              
              // Sanitize possible recursive objects coming from backend
            if (fileObj && typeof fileObj.path === 'object' && fileObj.path !== null) {
              fileObj.path = fileObj.path.path || '';
              recursiveFixCount++;
            }
            if (fileObj && typeof fileObj.name === 'object' && fileObj.name !== null) {
              fileObj.name = fileObj.name.name || '';
              recursiveFixCount++;
            }
            return fileObj;
              
            } catch (fileError) {
              console.error(`💥 Error processing file line ${index + 1}:`, fileError);
              console.error(`💥 Problematic line:`, line);
              return null;
            }
          })
          .filter(file => file && file.path && file.path.length > 0);
        
        if (recursiveFixCount > 0) {
        console.info(`🔧 Sanitized ${recursiveFixCount} recursive object fields from list_archive output`);
      }

      console.log(`🔍 Successfully processed ${files.length} files from ${fileLines.length} file lines`);
        
        return {
          success: true,
          files: files,
          output: result.output
        };
      } else {
        console.error('❌ Archive listing failed:', result.error);
        return {
          success: false,
          error: result.error
        };
      }
    } catch (error) {
      console.error('💥 Tauri list command failed:', error);
      return {
        success: false,
        error: error.toString()
      };
    }
  }

  /**
   * Delete file using Tauri
   */
  async deleteFile(filePath) {
    try {
      console.log('🗑️ Deleting file via Tauri:', filePath);
      
      const result = await invoke('delete_file', {
        filePath: filePath
      });
      
      if (result.success) {
        console.log('✅ File deleted successfully');
        return {
          success: true,
          output: result.output
        };
      } else {
        console.error('❌ File deletion failed:', result.error);
        return {
          success: false,
          error: result.error
        };
      }
    } catch (error) {
      console.error('💥 Tauri delete command failed:', error);
      return {
        success: false,
        error: error.toString()
      };
    }
  }
}

// Export singleton instance
const tauriBlitzArchEngine = new TauriBlitzArchEngine();
// Get real-time system metrics
const getSystemMetrics = async () => {
  try {
    const result = await invoke('get_system_metrics');
    return result;
  } catch (error) {
    console.error('❌ Failed to get system metrics:', error);
    throw error;
  }
};

// Listen to archive progress events from Tauri backend
const listenToProgressEvents = async (onProgress) => {
  try {
    const unlisten = await listen('archive-progress', (event) => {
      console.log('📊 Progress event received:', event.payload);
      if (onProgress) {
        onProgress(event.payload);
      }
    });
    return unlisten; // Return unlisten function for cleanup
  } catch (error) {
    console.error('❌ Failed to listen to progress events:', error);
    throw error;
  }
};

// Export all functions
tauriBlitzArchEngine.getSystemMetrics = getSystemMetrics;
tauriBlitzArchEngine.listenToProgressEvents = listenToProgressEvents;

export default tauriBlitzArchEngine;
