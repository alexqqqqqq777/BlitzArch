/**
 * BlitzArch Engine Integration
 * Direct integration with Rust CLI for web environment
 */

// Check if we're in a Node.js environment (for development)
const isNodeEnvironment = typeof process !== 'undefined' && process.versions && process.versions.node;

class BlitzArchEngine {
  constructor() {
    this.enginePath = '../target/release/blitzarch'; // Path to compiled Rust binary
    this.isProcessing = false;
    this.currentProcess = null;
  }

  /**
   * Execute BlitzArch CLI command via backend API
   * @param {string[]} args - Command arguments
   * @returns {Promise<{success: boolean, output?: string, error?: string}>}
   */
  async executeCommand(args) {
    return this.executeViaAPI(args);
  }

  /**
   * Execute command via backend API
   */
  async executeViaAPI(args) {
    try {
      const apiUrl = process.env.NODE_ENV === 'development' 
        ? 'http://localhost:3001/api/blitzarch'
        : '/api/blitzarch';
        
      const response = await fetch(apiUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ args }),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const result = await response.json();
      return result;
    } catch (error) {
      console.error('API call failed:', error);
      return { success: false, error: error.message };
    }
  }

  /**
   * Create archive from files
   */
  async createArchive(files, archiveName, outputDir, options = {}) {
    const {
      compressionLevel = 3,
      password = null,
      bundleSize = 32,
      useKatana = true,
      codecThreads = 0
    } = options;

    const args = ['create'];
    
    // Katana is enabled by default, use --no-katana to disable
    if (!useKatana) {
      args.push('--no-katana');
    }
    
    // Required parameters
    args.push('--output', `${outputDir}/${archiveName}.blz`);
    args.push('--bundle-size', bundleSize.toString());
    
    // Optional parameters
    if (compressionLevel !== 3) {
      args.push('--level', compressionLevel.toString());
    }
    
    if (codecThreads > 0) {
      args.push('--codec-threads', codecThreads.toString());
    }
    
    if (password) {
      args.push('--password', password);
    }

    // Add input files at the end
    files.forEach(file => {
      args.push(file);
    });

    console.log('Creating archive with args:', args);
    this.isProcessing = true;
    
    try {
      const result = await this.executeCommand(args);
      return result;
    } finally {
      this.isProcessing = false;
      this.currentProcess = null;
    }
  }

  /**
   * Extract archive
   */
  async extractArchive(archivePath, destinationPath, options = {}) {
    const { password = null } = options;

    const args = ['extract', archivePath];
    
    if (destinationPath) {
      args.push('--output', destinationPath);
    }
    
    if (password) {
      args.push('--password', password);
    }

    console.log('Extracting archive with args:', args);
    this.isProcessing = true;
    
    try {
      const result = await this.executeCommand(args);
      return result;
    } finally {
      this.isProcessing = false;
      this.currentProcess = null;
    }
  }

  /**
   * List archive contents
   */
  async listArchive(archivePath) {
    const args = ['list', archivePath];

    console.log('Listing archive with args:', args);
    
    try {
      const result = await this.executeCommand(args);
      if (result.success) {
        // Parse the output to get file list
        const files = result.output
          .split('\n')
          .filter(line => line.trim())
          .map(line => {
            // Parse BlitzArch list output format
            const parts = line.trim().split(/\s+/);
            return {
              name: parts[parts.length - 1],
              size: parts[0] || '0',
              compressed: parts[1] || '0',
              ratio: parts[2] || '0%'
            };
          });
        
        return { success: true, files };
      }
      return result;
    } catch (error) {
      return { success: false, error: error.message };
    }
  }

  /**
   * Get engine status and version
   */
  async getStatus() {
    try {
      const apiUrl = process.env.NODE_ENV === 'development' 
        ? 'http://localhost:3001/api/status'
        : '/api/status';
        
      const response = await fetch(apiUrl);
      const result = await response.json();
      return result;
    } catch (error) {
      return {
        available: false,
        version: null,
        error: error.message
      };
    }
  }

  /**
   * Cancel current operation
   */
  cancelOperation() {
    if (this.currentProcess) {
      this.currentProcess.kill('SIGTERM');
      this.currentProcess = null;
      this.isProcessing = false;
      return true;
    }
    return false;
  }

  /**
   * Check if engine is currently processing
   */
  isEngineProcessing() {
    return this.isProcessing;
  }
}

// Export singleton instance
export const blitzArchEngine = new BlitzArchEngine();
export default blitzArchEngine;
