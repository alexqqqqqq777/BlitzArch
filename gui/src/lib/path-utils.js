/**
 * Path utilities for BlitzArch
 * Smart path determination for archive creation
 * Browser-compatible implementation
 */

// Browser-compatible path utilities
const pathUtils = {
  resolve: (...paths) => {
    let resolvedPath = '';
    let isAbsolute = false;
    
    for (let i = paths.length - 1; i >= 0 && !isAbsolute; i--) {
      const path = paths[i];
      if (path) {
        resolvedPath = path + '/' + resolvedPath;
        isAbsolute = path.charAt(0) === '/';
      }
    }
    
    if (!isAbsolute) {
      resolvedPath = '/' + resolvedPath;
    }
    
    // Normalize the path
    return pathUtils.normalize(resolvedPath);
  },
  
  dirname: (path) => {
    if (!path) return '.';
    const normalizedPath = path.replace(/\/+$/, ''); // Remove trailing slashes
    const lastSlash = normalizedPath.lastIndexOf('/');
    if (lastSlash === -1) return '.';
    if (lastSlash === 0) return '/';
    return normalizedPath.slice(0, lastSlash);
  },
  
  basename: (path, ext) => {
    if (!path) return '';
    const normalizedPath = path.replace(/\/+$/, ''); // Remove trailing slashes
    const lastSlash = normalizedPath.lastIndexOf('/');
    let base = lastSlash === -1 ? normalizedPath : normalizedPath.slice(lastSlash + 1);
    
    if (ext && base.endsWith(ext)) {
      base = base.slice(0, -ext.length);
    }
    
    return base;
  },
  
  parse: (path) => {
    const dir = pathUtils.dirname(path);
    const base = pathUtils.basename(path);
    const lastDot = base.lastIndexOf('.');
    
    return {
      dir,
      base,
      ext: lastDot !== -1 ? base.slice(lastDot) : '',
      name: lastDot !== -1 ? base.slice(0, lastDot) : base
    };
  },
  
  normalize: (path) => {
    if (!path) return '.';
    
    const isAbsolute = path.charAt(0) === '/';
    const parts = path.split('/').filter(part => part && part !== '.');
    const normalizedParts = [];
    
    for (const part of parts) {
      if (part === '..') {
        if (normalizedParts.length > 0 && normalizedParts[normalizedParts.length - 1] !== '..') {
          normalizedParts.pop();
        } else if (!isAbsolute) {
          normalizedParts.push('..');
        }
      } else {
        normalizedParts.push(part);
      }
    }
    
    const result = (isAbsolute ? '/' : '') + normalizedParts.join('/');
    return result || (isAbsolute ? '/' : '.');
  }
};

/**
 * Determine the optimal output directory for archive creation
 * Golden Standard Rules:
 * 1. Single file: archive goes to same directory as the file
 * 2. Multiple files from same directory: archive goes to that directory
 * 3. Multiple files from different directories: archive goes to common parent directory
 * 4. Mix of files and folders: archive goes to common parent directory
 */
export function determineOutputPath(filePaths) {
  if (!filePaths || filePaths.length === 0) {
    throw new Error('No files provided');
  }

  // Normalize all paths
  const normalizedPaths = filePaths.map(p => pathUtils.resolve(p));

  if (normalizedPaths.length === 1) {
    // Single file or folder - use its parent directory
    return pathUtils.dirname(normalizedPaths[0]);
  }

  // Multiple files - find common parent directory
  const commonParent = findCommonParentDirectory(normalizedPaths);
  return commonParent;
}

/**
 * Generate a smart archive name based on input files
 */
export function generateArchiveName(filePaths) {
  if (!filePaths || filePaths.length === 0) {
    return 'archive';
  }

  const normalizedPaths = filePaths.map(p => pathUtils.resolve(p));

  if (normalizedPaths.length === 1) {
    // Single file or folder - use its name
    const baseName = pathUtils.basename(normalizedPaths[0]);
    // Remove extension if it's a file
    const nameWithoutExt = pathUtils.parse(baseName).name;
    return nameWithoutExt || 'archive';
  }

  // Multiple files - try to find a meaningful name
  const commonParent = findCommonParentDirectory(normalizedPaths);
  const parentName = pathUtils.basename(commonParent);
  
  if (parentName && parentName !== '/' && parentName !== '.') {
    return parentName;
  }

  // Fallback to timestamp-based name
  const timestamp = new Date().toISOString().slice(0, 19).replace(/[:.]/g, '-');
  return `archive-${timestamp}`;
}

/**
 * Find the common parent directory of multiple paths
 */
function findCommonParentDirectory(paths) {
  if (paths.length === 1) {
    return pathUtils.dirname(paths[0]);
  }

  // Split paths into segments
  const pathSegments = paths.map(p => p.split('/'));
  
  // Find the shortest path to limit comparison
  const minLength = Math.min(...pathSegments.map(segments => segments.length));
  
  // Find common prefix
  let commonLength = 0;
  for (let i = 0; i < minLength; i++) {
    const segment = pathSegments[0][i];
    if (pathSegments.every(segments => segments[i] === segment)) {
      commonLength = i + 1;
    } else {
      break;
    }
  }

  if (commonLength === 0) {
    // No common parent found, use root or current directory
    return '/';
  }

  // Reconstruct the common parent path
  const commonSegments = pathSegments[0].slice(0, commonLength);
  return commonSegments.join('/') || '/';
}

/**
 * Validate that the output directory is writable
 */
export async function validateOutputDirectory(dirPath) {
  try {
    // Check if directory exists and is writable
    const response = await fetch('/api/validate-directory', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ dirPath })
    });
    
    const result = await response.json();
    return result;
  } catch (error) {
    return {
      valid: false,
      error: error.message
    };
  }
}

/**
 * Get file/directory information
 */
export async function getFileInfo(filePath) {
  try {
    const response = await fetch('/api/file-info', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ filePath })
    });
    
    const result = await response.json();
    return result;
  } catch (error) {
    return {
      success: false,
      error: error.message
    };
  }
}

/**
 * Create the complete archive path with extension
 */
export function createArchivePath(outputDir, archiveName) {
  // Ensure the archive name has .blz extension
  const nameWithExtension = archiveName.endsWith('.blz') 
    ? archiveName 
    : `${archiveName}.blz`;
  
  return pathUtils.normalize(outputDir + '/' + nameWithExtension);
}

/**
 * Format file size for display
 */
export function formatFileSize(bytes) {
  if (bytes === 0) return '0 B';
  
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

/**
 * Get relative path for display
 */
export function getDisplayPath(fullPath, basePath = null) {
  if (!basePath) {
    return pathUtils.basename(fullPath);
  }
  
  if (basePath) {
    // Simple relative path calculation for browser
    if (fullPath.startsWith(basePath)) {
      return fullPath.slice(basePath.length).replace(/^\/+/, '');
    }
    return pathUtils.basename(fullPath);
  } else {
    return pathUtils.basename(fullPath);
  }
}
