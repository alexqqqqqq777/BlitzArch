import React, { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Cpu,
  HardDrive,
  CheckCircle,
  XCircle
} from 'lucide-react';

import MainWorkspace from '../components/archiver/MainWorkspace';
import ControlDashboard from '../components/archiver/ControlDashboard';
import MetricsPanel from '../components/archiver/MetricsPanel';
import TaskProgress from '../components/archiver/TaskProgress';
import SystemStatus from '../components/archiver/SystemStatus';
import ResultModal from '../components/archiver/ResultModal';
import tauriBlitzArchEngine from '../lib/tauri-engine.js';
import { invoke } from '@tauri-apps/api/core';

import { determineOutputPath, generateArchiveName, createArchivePath, validateOutputDirectory } from '../lib/path-utils.js';

// New stylish icon with lightning bolt
const BlitzIcon = (props) => (
  <svg {...props} viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
    <defs>
      <linearGradient id="blitz-gradient" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" stopColor="#0891b2" />
        <stop offset="50%" stopColor="#06b6d4" />
        <stop offset="100%" stopColor="#67e8f9" />
      </linearGradient>
    </defs>
    <path 
      d="M13 2L4.09 12.97C3.52 13.79 4.07 15 5.09 15H11V22L19.91 11.03C20.48 10.21 19.93 9 18.91 9H13V2Z" 
      fill="url(#blitz-gradient)" 
    />
    <path 
      d="M12 7L8 12H12V17L16 12H12V7Z" 
      fill="rgba(255,255,255,0.3)" 
    />
  </svg>
);

// Parse katana/engine textual output to extract stats
// Example string: "Archive complete | Files: 70002 | Time: 4.2s | Ratio: 4.34:1 | Speed: 201.1 MB/s"
const parseEngineStats = (outputStr = '') => {
  // Remove ANSI codes and extra spaces
  const cleanStr = outputStr.replace(/\u001b\[[0-9;]*m/g, '').replace(/\s+/g, ' ');

  const stats = {};

  const filesMatch = cleanStr.match(/Files:\s*(\d+[\d,]*)/i);
  if (filesMatch) stats.files = parseInt(filesMatch[1], 10);

  // Support "Time: 4.2 s" and "Time: 4.2s"
  const timeMatch = cleanStr.match(/Time:\s*([\d.]+)\s*s/i);
  if (timeMatch) stats.duration = parseFloat(timeMatch[1]);

  const ratioMatch = cleanStr.match(/Ratio:\s*([\d.]+)\s*:?\s*1?/i);
  if (ratioMatch) {
    const raw = ratioMatch[1];
    // if number less than 1 => this is compression coefficient (compressed/original), then invert
    const numeric = parseFloat(raw);
    if (!isNaN(numeric)) {
      // Don't invert; output as is
      if (numeric < 1) {
        stats.compressionRatio = `${numeric.toFixed(3)}:1`;
      } else {
        stats.compressionRatio = `${numeric.toFixed(2)}:1`;
      }
    } else {
      stats.compressionRatio = raw;
    }
  }

  const speedMatch = cleanStr.match(/Speed:\s*([\d.]+\s*[A-Z]+\/s)/i);
  if (speedMatch) stats.speed = speedMatch[1];

    return stats;
};

// Helper function to generate archive name from File objects
const generateArchiveNameFromFiles = (files) => {
  if (!files || files.length === 0) {
    return 'archive';
  }
  
  if (files.length === 1) {
    // Single file - use its name without extension
    const fileName = files[0].name || files[0];
    const lastDot = fileName.lastIndexOf('.');
    return lastDot !== -1 ? fileName.slice(0, lastDot) : fileName;
  }
  
  // Multiple files - use timestamp-based name
  const timestamp = new Date().toISOString().slice(0, 19).replace(/[:.]/g, '-');
  return `archive-${timestamp}`;
};

// Tauri-based archive creation with real file paths
// Smart extraction with path duplication prevention
const extractWithSmartPathHandling = async (archive, destinationPath, options = {}) => {
  try {
    // Get list of files in archive
    const listResult = await tauriBlitzArchEngine.listArchive(archive.path);
    
    if (!listResult.success || !listResult.files || listResult.files.length === 0) {
      console.warn('⚠️ Cannot analyze archive contents, using standard extraction');
      return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
    }
    
    const filePaths = listResult.files;
    console.log('📋 Archive file paths:', filePaths);
    
    // Find common root path of all files
    const commonRoot = findCommonRootPath(filePaths);
    console.log('🌳 Common root path:', commonRoot);
    
    // Check if we need to avoid path duplication
    const needsSmartExtraction = commonRoot && (
      destinationPath.includes(commonRoot) || 
      commonRoot.includes(destinationPath.split('/').pop())
    );
    
    if (needsSmartExtraction) {
      console.log('⚠️ Path duplication detected, using smart extraction');
      
      // Create temporary folder for extraction
      const tempDir = `${destinationPath}/.blitzarch_temp_${Date.now()}`;
      console.log('📁 Extracting to temp directory:', tempDir);
      
      // Extract to temporary folder
      const extractResult = await tauriBlitzArchEngine.extractArchive(
        archive.path, 
        tempDir, 
        options
      );
      
      if (!extractResult.success) {
        return extractResult;
      }
      
      // Move files from temporary folder to target, avoiding duplication
      console.log('🔄 Moving files to final destination...');
      const moveResult = await moveFilesSmartly(tempDir, destinationPath, commonRoot);
      
      // Clean up temporary folder
      await cleanupTempDirectory(tempDir);
      
      return moveResult;
    } else {
      console.log('✅ No path duplication detected, using standard extraction');
      return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
    }
  } catch (error) {
    console.error('❌ Error in smart extraction:', error);
    // Fallback to standard extraction
    return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
  }
};

// Find common root path for array of paths
const findCommonRootPath = (paths) => {
  if (!paths || paths.length === 0) return null;
  if (paths.length === 1) {
    // For single file return its directory
    const path = paths[0];
    const lastSlash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return lastSlash > 0 ? path.substring(0, lastSlash) : null;
  }
  
  // For multiple files find common prefix
  const firstPath = paths[0];
  let commonPath = '';
  
  for (let i = 0; i < firstPath.length; i++) {
    const char = firstPath[i];
    if (paths.every(path => path[i] === char)) {
      commonPath += char;
    } else {
      break;
    }
  }
  
  // Trim to last slash
  const lastSlash = Math.max(commonPath.lastIndexOf('/'), commonPath.lastIndexOf('\\'));
  return lastSlash > 0 ? commonPath.substring(0, lastSlash) : null;
};

const createArchiveWithGoldenStandard = async (files, settings) => {
  const {
    compressionLevel = 3,
    password = null,
    bundleSize = 32,
    threads = 0,
    codecThreads = 0, // not used in backend yet, but keep for compatibility
    memoryBudget = 0
  } = settings;
  try {
    const archiveName = generateArchiveNameFromFiles(files);
    
    console.log('🎯 Tauri Archive Creation:');
    console.log('📦 Archive Name:', archiveName);
    console.log('📋 Input Files:', files);
    
    // Get file paths - in Tauri we have access to real paths!
    let filePaths = [];
    let outputDir = null;
    
    if (files[0] && files[0].path) {
      // Files from Tauri file dialog have real paths
      filePaths = files.map(f => f.path);
      // Get parent directory of first file for output
      outputDir = await tauriBlitzArchEngine.getParentDirectory(files[0].path);
    } else {
      // Fallback: use file names (for drag & drop)
      filePaths = files.map(f => f.name || f);
      // Use Downloads as fallback
      outputDir = await tauriBlitzArchEngine.getParentDirectory('~/Downloads/temp') || '~/Downloads';
    }
    
    console.log('📁 File paths:', filePaths);
    console.log('🎯 Output directory:', outputDir);
    
    // Use Tauri command to create archive
    const result = await tauriBlitzArchEngine.createArchive(
      filePaths,
      archiveName,
      outputDir,
      {
        compressionLevel,
        password,
        bundleSize,
        memoryBudget,
        codecThreads,
        threads
      }
    );
    
    if (result.success) {
      console.log('✅ Archive created successfully:', result.archivePath);
      return { 
        success: true, 
        output: result.output,
        archivePath: result.archivePath,
        archiveName: archiveName,
        outputDir: outputDir,
        stats: result.stats || null
      };
    } else {
      console.error('❌ Failed to create archive:', result.error);
      return { success: false, error: result.error };
    }
  } catch (error) {
    console.error('💥 Error in archive creation:', error);
    return { success: false, error: error.message };
  }
};

export default function BlitzArch() {
  const [activeMode, setActiveMode] = useState('create');
  const [isProcessing, setIsProcessing] = useState(false);
  const startProcessing = (type) => {
    setFinalMessage(null);
    setIsProcessing(true);
    setProcessingType(type);
  }
  const [processingType, setProcessingType] = useState(null);
  const [progress, setProgress] = useState(0);
  const [speed, setSpeed] = useState(0);
  const [finalMessage, setFinalMessage] = useState(null);
  // Ref to keep last progress data
  const lastProgressRef = useRef(null);
  // Result modal state
  const [isResultModalOpen, setIsResultModalOpen] = useState(false);
  const [resultData, setResultData] = useState(null);
  
  // Rich metrics state
  const [processedFiles, setProcessedFiles] = useState(0);
  const [totalFiles, setTotalFiles] = useState(0);
  const [processedBytes, setProcessedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);
  const [completedShards, setCompletedShards] = useState(0);
  const [totalShards, setTotalShards] = useState(0);
  const [elapsedTime, setElapsedTime] = useState(0);
  const [etaSeconds, setEtaSeconds] = useState(0);
  const [compressionRatio, setCompressionRatio] = useState(null);
  const [selectedArchive, setSelectedArchive] = useState(null);
  const [archives, setArchives] = useState([]);
  const [logs, setLogs] = useState([]);
  const [settings, setSettings] = useState({
    preset: 'balanced',
    compressionLevel: 3,        // README default level (balanced profile)
    bundleSize: 0,             // Auto bundle size (balanced profile default)
    password: '',
    useEncryption: false,
    threads: 0,                // Auto threads
    codecThreads: 0,            // Auto codec threads
    memoryBudget: 0            // Auto mem budget
  });

  // Listen to archive progress events for real-time UI updates
  useEffect(() => {
    let unlistenFunction = null;
    
    const setupProgressListener = async () => {
      try {
        unlistenFunction = await tauriBlitzArchEngine.listenToProgressEvents((progressData) => {
          console.log('📊 Progress update received:', progressData);
          
          // Update basic progress and speed from real-time events
          lastProgressRef.current = progressData; // save last event
      setProgress(Math.min(progressData.progress ?? 0, 100));
          setSpeed(progressData.speed || 0);
          
          // Update all rich metrics
          setProcessedFiles(progressData.processed_files || 0);
          setTotalFiles(progressData.total_files || 0);
          setProcessedBytes(progressData.processed_bytes || 0);
          setTotalBytes(progressData.total_bytes || 0);
          setCompletedShards(progressData.completed_shards || 0);
          setTotalShards(progressData.total_shards || 0);
          setElapsedTime(progressData.elapsed_time || 0);
          setEtaSeconds(progressData.eta_seconds || 0);
          setCompressionRatio(progressData.compression_ratio || null);
          
          // Update processing type based on operation
          if (progressData.operation) {
            setProcessingType(progressData.operation);
          }
          
          // Handle completion
          if (progressData.completed) {
            setIsProcessing(false);
            setFinalMessage(progressData.message || null);
            if (progressData.error) {
              addLog(`Operation failed: ${progressData.error}`, 'error');
            } else {
              addLog(progressData.message || 'Operation completed successfully', 'success');
            }
          } else {
            // Update log with progress message
            if (progressData.message) {
              addLog(progressData.message, 'info');
            }
          }
        });
        
        console.log('✅ Progress event listener setup complete');
      } catch (error) {
        console.error('❌ Failed to setup progress listener:', error);
      }
    };
    
    setupProgressListener();
    
    // Cleanup listener on unmount
    return () => {
      if (unlistenFunction) {
        unlistenFunction();
      }
    };
  }, []);

  // Global drag-out handler for extracting files from archive
  useEffect(() => {
    const handleGlobalDragOver = (event) => {
      // Allow drag-out to external destinations
      event.preventDefault();
      event.dataTransfer.dropEffect = 'copy';
    };

    const handleGlobalDrop = async (event) => {
      event.preventDefault();
      
      try {
        const dragDataStr = event.dataTransfer.getData('application/json');
        if (!dragDataStr) return;
        
        const dragData = JSON.parse(dragDataStr);
        if (dragData.type !== 'blitzarch-file') return;
        
        console.log('🎯 Drag-out detected:', dragData);
        addLog(`🎯 Extracting file: ${dragData.fileName}`, 'info');
        
        // Determine target directory (Downloads folder as fallback)
        const downloadsDir = await invoke('get_downloads_path')
          .catch(() => '/Users/oleksandr/Downloads'); // Fallback
        
        // Extract file using our new command
        const result = await invoke('drag_out_extract', {
  // Pass both key variations (snake_case and camelCase) for compatibility
  archive_path: dragData.archivePath,
  archivePath: dragData.archivePath,
  file_path: dragData.filePath,
  filePath: dragData.filePath,
  target_dir: downloadsDir,
  targetDir: downloadsDir,
  password: settings.useEncryption ? settings.password : null
});
        
        if (result.success) {
          addLog(`✅ File extracted successfully: ${result.archive_path}`, 'success');
        } else {
          addLog(`❌ Extraction error: ${result.error}`, 'error');
        }
        
      } catch (error) {
        console.error('❌ Drag-out error:', error);
        addLog(`❌ Drag-out error: ${error.message}`, 'error');
      }
    };
    
    // Add global event listeners
    document.addEventListener('dragover', handleGlobalDragOver);
    document.addEventListener('drop', handleGlobalDrop);
    
    // Cleanup
    return () => {
      document.removeEventListener('dragover', handleGlobalDragOver);
      document.removeEventListener('drop', handleGlobalDrop);
    };
  }, [settings.useEncryption, settings.password]);

  const addLog = (message, type = 'info') => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-9), { message, type, timestamp }]);
  };

  const handleCreateArchive = useCallback(async (files) => {
    if (!files || files.length === 0) {
      addLog('No files selected for archiving', 'error');
      return;
    }

    startProcessing('create');
    setProgress(0);
    setSpeed(0);
    
    addLog(`🚀 Starting archive creation from ${files.length} files...`);
    addLog(`📋 Files: ${files.map(f => f.name || f).join(', ')}`);
    
    try {
      const result = await createArchiveWithGoldenStandard(files, settings);
      const formatSpeed = (val) => {
            if(!val) return undefined;
            let mbs = val;
            // if value looks like bytes/sec convert
            if (val > 1000) mbs = val / (1024*1024);
            return `${mbs.toFixed(1)} MB/s`;
          };
          const formatRatio = (val) => {
            if(!val) return undefined;
             // Don't invert, just round
             if (val > 0 && val < 1) return `${val.toFixed(3)}:1`;
            if (val > 1 && val < 1000) return `${val.toFixed(2)}:1`;
            return undefined;
          };
          // Формируем объект статистики
          let statsObj = {};
          if (result.success) {
            console.log('🔍 result.stats from backend:', result.stats);
            console.log('🔍 result.output from backend:', result.output);
            
            if (result.stats && Object.keys(result.stats).length > 0) {
              statsObj = result.stats;
              console.log('✅ Using backend stats:', statsObj);
            } else {
              // Parse engine text output
              statsObj = parseEngineStats(result.output);
              console.log('🔍 Parsed stats from output:', statsObj);
            }

        // If backend didn't return statistics (may be empty object), try to supplement it with data from last progress event
        if (!statsObj || Object.keys(statsObj).length === 0 || (!statsObj.files && !statsObj.duration)) {
          const pd = lastProgressRef.current;
          if (pd) {
            statsObj = {
              files: pd.total_files ?? pd.processed_files,
              duration: pd.duration ?? pd.elapsed,
              compressionRatio: formatRatio(pd.compression_ratio),
              speed: formatSpeed(pd.speed),
              size: pd.total_bytes ?? pd.processed_bytes
            };
          }
        }
      }
      
      if (result.success) {
        addLog(`✅ Archive created successfully!`, 'success');
        addLog(`📦 Archive name: ${result.archiveName}`, 'success');
        addLog(`📁 Location: ${result.outputDir}`, 'success');
        addLog(`🗾️ Full path: ${result.archivePath}`, 'success');
        
        // Add the created archive to the list
        const newArchive = {
          id: Date.now().toString(),
          name: result.archiveName + '.blz',
          path: result.archivePath,
          size: 'Unknown',
          created: new Date().toISOString(),
          files: files.length
        };
        setArchives(prev => [newArchive, ...prev]);
        setResultData({
          type: 'create_success',
          message: 'Archive created successfully',
          outputPath: result.archivePath,
          stats: statsObj
        });
        setIsResultModalOpen(true);
      } else {
        addLog(`❌ Archive creation error: ${result.error}`, 'error');
        setResultData({
          type: 'create_error',
          message: result.error,
          error: result.error
        });
        setIsResultModalOpen(true);
      }
    } catch (error) {
      addLog(`💥 Unexpected error: ${error.message}`, 'error');
      setResultData({
        type: 'create_error',
        message: error.message,
        error: error.message
      });
      setIsResultModalOpen(true);
    } finally {
      setIsProcessing(false);
      setProgress(100);
    }
  }, [settings, addLog]);

  const handleLoadArchive = async (archivePath) => {
    if (!archivePath) {
      addLog('No archive selected', 'warning');
      return;
    }
    
    // Extract string path from archivePath (could be object or string)
    let actualPath;
    if (typeof archivePath === 'string') {
      actualPath = archivePath;
    } else if (archivePath.path) {
      actualPath = archivePath.path;
    } else if (Array.isArray(archivePath) && archivePath.length > 0) {
      actualPath = archivePath[0].path || archivePath[0];
    } else {
      actualPath = archivePath.toString();
    }
    
    addLog(`Loading archive: ${actualPath}`, 'info');
    
    try {
      const result = await tauriBlitzArchEngine.listArchive(actualPath);
      
      if (result.success) {
        const archiveObj = {
          name: actualPath.split('/').pop(),
          path: actualPath,
          files: result.files, // Use real data from engine
          encrypted: false
        };
        
        setSelectedArchive(archiveObj);
        setActiveMode('browse');
        addLog(`Archive loaded: ${result.files.length} files found`, 'success');
      } else {
        addLog(`Failed to load archive: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`Error loading archive: ${error.message}`, 'error');
    }
  };

  const handleExtractArchive = async (extractRequest = []) => {

    
    // Initialize statsObj to avoid ReferenceError
    let statsObj = {};
    let batchHadPasswordError = false; // Flag to track password errors in batch mode
    
    // Handle different input formats:
    // 1. Legacy: array of file paths (batch mode)
    // 2. New: object with {archivePath, selectedFiles} from ArchiveExplorer
    // 3. Empty: single archive from selectedArchive
  
    let isBatchMode = false;
    let archivePathToExtract = null;
    let specificFiles = null;
  
    if (Array.isArray(extractRequest)) {
      // Legacy format - batch mode with archive paths
      isBatchMode = extractRequest.length > 0;
    } else if (extractRequest && typeof extractRequest === 'object' && extractRequest.archivePath) {
      // New format from ArchiveExplorer
      archivePathToExtract = extractRequest.archivePath;
      specificFiles = extractRequest.selectedFiles;
      isBatchMode = false; // This is single archive with specific files
    }
  
    const selectedFiles = Array.isArray(extractRequest) ? extractRequest : [];
    
    if (!isBatchMode && !selectedArchive && !archivePathToExtract) {
      addLog('No archive selected for extraction', 'warning');
      return;
    }
    
    // Handle batch extraction of multiple archives
    if (isBatchMode) {
      // --- Batch list sanitation ----------------------------------------------------
      const archiveExts = ['.blz']; // supported archives for batch-mode
      const uniqueByPath = Array.from(new Set(selectedFiles.map(f => (f.path || f).toString())));
      const sanitized = uniqueByPath.filter(p => archiveExts.some(ext => p.toLowerCase().endsWith(ext)));

      if (sanitized.length === 0) {
        addLog('No valid archives found for extraction', 'warning');
        setIsProcessing(false);
        setProcessingType(null);
        return;
      }

      addLog(`Starting batch extraction of ${sanitized.length} archives`, 'info');
      
      for (let i = 0; i < sanitized.length; i++) {
        const filePathRaw = sanitized[i];
        const file = typeof filePathRaw === 'string' ? { path: filePathRaw } : filePathRaw;
        const archivePath = file.path || filePathRaw;
        
        // Automatically determine destination: parent directory of the archive
        const archiveDir = archivePath.substring(0, archivePath.lastIndexOf('/'));
        const archiveName = archivePath.substring(archivePath.lastIndexOf('/') + 1);
        
        addLog(`[${i + 1}/${sanitized.length}] Extracting ${archiveName} to its parent directory`, 'info');
        
        try {
          // Use automatic strip_components for golden standard UX
          const result = await tauriBlitzArchEngine.extractArchive(
            archivePath,
            archiveDir, // Extract to archive's parent directory
            { 
              password: settings.useEncryption ? settings.password : null,
              autoStripComponents: true // dynamic strip
            }
          );
          
          if (result.success) {
            // Try to get ready statistics object from engine
            statsObj = result.stats || {};
            // If engine doesn't return stats yet, parse text output
            if (!statsObj || Object.keys(statsObj).length === 0) {
              const parsed = parseEngineStats(result.output);
              if (parsed && Object.keys(parsed).length > 0) {
                statsObj = parsed;
              }
            }
            addLog(`[${i + 1}/${sanitized.length}] ✅ ${archiveName} extracted successfully`, 'success');
          } else {
            addLog(`[${i + 1}/${sanitized.length}] ❌ Failed to extract ${archiveName}: ${result.error}`, 'error');
            
            // Show ResultModal immediately for password errors
            if (result.error && result.error.includes('password required')) {
              batchHadPasswordError = true; // Set flag to prevent batch completion modal
              
              // Create retry function that will retry extraction with new password
              const retryExtraction = async (newPassword) => {
                setIsProcessing(true);
                startProcessing('extract');
                setIsResultModalOpen(false); // Close modal during retry
                
                try {
                  const retryResult = await tauriBlitzArchEngine.extractArchive(
                    archivePath,
                    archiveDir,
                    { 
                      password: newPassword, // Use new password
                      autoStripComponents: true
                    }
                  );
                  

                  
                  if (retryResult.success) {
                    // Success - show success modal
                    const successData = {
                      type: 'extract_success',
                      message: `Archive ${archiveName} extracted successfully`,
                      outputPath: archiveDir,
                      stats: retryResult.stats || {}
                    };
                    setResultData(successData);
                    setIsResultModalOpen(true);
                    addLog(`✅ ${archiveName} extracted successfully with password`, 'success');
                    return true; // Success
                  } else {
                    // Still failed - show error again
                    const errorData = {
                      type: 'password_error',
                      message: `Password required for ${archiveName}`,
                      error: retryResult.error,
                      onRetry: retryExtraction // Allow another retry
                    };
                    setResultData(errorData);
                    setIsResultModalOpen(true);
                    addLog(`❌ ${archiveName} retry failed: ${retryResult.error}`, 'error');
                    return false; // Still failed
                  }
                } catch (error) {
                  // Exception during retry
                  const errorData = {
                    type: 'extract_error',
                    message: `Error retrying ${archiveName}`,
                    error: error.message
                  };
                  setResultData(errorData);
                  setIsResultModalOpen(true);
                  addLog(`❌ ${archiveName} retry error: ${error.message}`, 'error');
                  return false;
                } finally {
                  setIsProcessing(false);
                  setProcessingType(null);
                }
              };
              
              const passwordErrorData = {
                type: 'password_error', // Use password_error type to show input field
                message: `Password required for ${archiveName}`,
                error: result.error,
                onRetry: retryExtraction // Pass retry function
              };
              setResultData(passwordErrorData);
              setIsResultModalOpen(true);
              
              setIsProcessing(false);
              setProcessingType(null);
              return; // Stop batch processing on password error
            }
          }
        } catch (error) {
          addLog(`[${i + 1}/${sanitized.length}] ❌ Error extracting ${archiveName}: ${error.message}`, 'error');
        }
        
        // Update progress
        // Batch extract manual progress – clamp to 100%
        setProgress(Math.min(Math.round(((i + 1) / sanitized.length) * 100), 100));
      }
      
      addLog(`Batch extraction completed: ${sanitized.length} archives processed`, 'info');
      
      // Show ResultModal for batch extraction results only if no password error occurred
      if (!batchHadPasswordError) {
        const resultData = {
          type: 'extract_success', // TODO: handle mixed success/error results
          message: `Batch extraction completed: ${sanitized.length} archives processed`,
          outputPath: 'Multiple locations', // TODO: collect all output paths
          stats: statsObj // Use stats from last successful extraction
        };
        setResultData(resultData);
        setIsResultModalOpen(true);
      } else {
      }
      
      setIsProcessing(false);
      setProcessingType(null);
      return;
    }
    
    // Handle extraction from ArchiveExplorer (specific files from archive)
    if (archivePathToExtract) {
      // For ArchiveExplorer extraction, extract to archive's parent directory
      const archiveDir = archivePathToExtract.substring(0, archivePathToExtract.lastIndexOf('/')) || './';
      const archiveName = archivePathToExtract.substring(archivePathToExtract.lastIndexOf('/') + 1);
      
      if (specificFiles && specificFiles.length > 0) {
        addLog(`Extracting ${specificFiles.length} selected files from ${archiveName}`, 'info');
      } else {
        addLog(`Extracting all files from ${archiveName}`, 'info');
      }
      addLog(`Destination: ${archiveDir}`, 'info');
      
      try {
        const result = await tauriBlitzArchEngine.extractArchive(
          archivePathToExtract,
          archiveDir,
          { 
            password: settings.useEncryption ? settings.password : null,
            autoStripComponents: true, // test auto calculation
            specificFiles: specificFiles // Extract only selected files if specified
          }
        );
        
        if (result.success) {
          addLog(`Files extracted successfully from ${archiveName}`, 'success');
          // Формируем объект статистики
         let statsObj = {};
         if (result.stats && Object.keys(result.stats).length > 0) {
           statsObj = result.stats;
         } else {
           statsObj = parseEngineStats(result.output);
         }
         if (!statsObj || Object.keys(statsObj).length === 0 || (!statsObj.files && !statsObj.duration)) {
           const pd = lastProgressRef.current;
           if (pd) {
             statsObj = {
               files: pd.total_files ?? pd.processed_files,
               duration: pd.duration ?? pd.elapsed,
               compressionRatio: formatRatio(pd.compression_ratio),
               speed: formatSpeed(pd.speed),
               size: pd.total_bytes ?? pd.processed_bytes
             };
           }
         }
          const resultData = {
            type: 'extract_success',
            message: 'Files extracted successfully',
            outputPath: archiveDir,
            stats: statsObj
          };
          setResultData(resultData);
          setIsResultModalOpen(true);
        } else {
          addLog(`Failed to extract files from ${archiveName}: ${result.error}`, 'error');
          const errorData = {
            type: 'extract_error',
            message: result.error,
            error: result.error
          };
          setResultData(errorData);
          setIsResultModalOpen(true);
        }
      } catch (error) {
        addLog(`Error extracting files: ${error.message}`, 'error');
        const catchErrorData = {
          type: 'extract_error',
          message: error.message,
          error: error.message
        };
        setResultData(catchErrorData);
        setIsResultModalOpen(true);
      } finally {
        setIsProcessing(false);
        setProcessingType(null);
        setProgress(100);
      }
      return;
    }
    
    // Single archive mode - show destination dialog
    let destinationPath;
    try {
      // Use Tauri dialog to select destination folder
      const { open } = await import('@tauri-apps/plugin-dialog');
      
      destinationPath = await open({
        directory: true,
        multiple: false,
        title: 'Select extraction destination folder'
      });
      
      if (!destinationPath) {
        addLog('No destination folder selected', 'warning');
        setIsProcessing(false);
        setProcessingType(null);
        return;
      }
    } catch (error) {
      destinationPath = './'; // fallback
    }
    
    addLog(`Starting extraction: ${selectedArchive.name}`, 'info');
    addLog(`Extraction destination: ${destinationPath}`, 'info');
    
    try {
      // Use automatic strip_components calculation for golden standard UX
      const result = await tauriBlitzArchEngine.extractArchive(
        selectedArchive.path,
        destinationPath,
        { 
          password: settings.useEncryption ? settings.password : null,
          autoStripComponents: true // dynamic strip
        }
      );
      
      if (result.success) {
        addLog(`Archive extracted successfully to: ${destinationPath}`, 'success');

        // Формируем объект статистики
         let statsObj = {};
         if (result.stats && Object.keys(result.stats).length > 0) {
           statsObj = result.stats;
         } else {
           statsObj = parseEngineStats(result.output);
         }
         if (!statsObj || Object.keys(statsObj).length === 0 || (!statsObj.files && !statsObj.duration)) {
           const pd = lastProgressRef.current;
           if (pd) {
             statsObj = {
               files: pd.total_files ?? pd.processed_files,
               duration: pd.duration ?? pd.elapsed,
               compressionRatio: formatRatio(pd.compression_ratio),
               speed: formatSpeed(pd.speed),
               size: pd.total_bytes ?? pd.processed_bytes
             };
           }
         }
        setResultData({
          type: 'extract_success',
          message: 'Files extracted successfully',
          outputPath: destinationPath,
          stats: statsObj
        });
        setIsResultModalOpen(true);
      } else {
        addLog(`Failed to extract archive: ${result.error}`, 'error');

        setResultData({
          type: 'extract_error',
          message: result.error || 'Archive extraction error',
          error: result.error
        });
        setIsResultModalOpen(true);
      }
    } catch (error) {
      addLog(`Error extracting archive: ${error.message}`, 'error');

      setResultData({
        type: 'extract_error',
        message: error.message || 'Archive extraction error',
        error: error.message
      });
      setIsResultModalOpen(true);
    } finally {
      setIsProcessing(false);
      setProcessingType(null);
      setProgress(100);
    }
  };

  const handleDeleteArchive = async (archivePath) => {
    try {
      const result = await tauriBlitzArchEngine.deleteFile(archivePath);
      
      if (result.success) {
        setArchives(prev => prev.filter(archive => archive.path !== archivePath));
        addLog(`Archive deleted: ${archivePath}`, 'success');
      } else {
        addLog(`Failed to delete archive: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`Error deleting archive: ${error.message}`, 'error');
    }
  };



  return (
    <>
    <div className="min-h-screen bg-gradient-to-br from-zinc-950 via-neutral-900 to-stone-950 font-sans">
      <div className="min-h-screen relative">
        {/* Grid overlay */}
        <div className="absolute inset-0 opacity-5">
          <div 
            className="w-full h-full" 
            style={{
              backgroundImage: `
                linear-gradient(rgba(255,255,255,0.1) 1px, transparent 1px),
                linear-gradient(90deg, rgba(255,255,255,0.1) 1px, transparent 1px)
              `,
              backgroundSize: '24px 24px'
            }}
          />
        </div>

        <div className="relative z-10 p-6 md:p-8">
          {/* Header */}
          <motion.div 
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            className="mb-10"
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div className="p-2 bg-gradient-to-br from-neutral-800 to-neutral-900 rounded-xl border border-neutral-700 shadow-lg">
                  <BlitzIcon className="w-10 h-10" />
                </div>
                <div>
                  <h1 className="text-3xl md:text-4xl font-extrabold text-white tracking-tight">
                    BlitzArch <span className="text-teal-400">Pro</span>
                  </h1>
                  <p className="text-neutral-400 text-sm md:text-base">Enterprise Archive Solution</p>
                </div>
              </div>
              
              <div className="hidden md:flex items-center gap-4">
                <div className="flex items-center gap-3 px-4 py-2 bg-neutral-800/50 rounded-lg border border-neutral-700">
                  <Cpu className="w-4 h-4 text-teal-400" />
                  <span className="text-neutral-300 text-sm font-mono">CPU: Ready</span>
                </div>
                <div className="flex items-center gap-3 px-4 py-2 bg-neutral-800/50 rounded-lg border border-neutral-700">
                  <HardDrive className="w-4 h-4 text-emerald-400" />
                  <span className="text-neutral-300 text-sm font-mono">Engine: Online</span>
                </div>
              </div>
            </div>
          </motion.div>

          {/* Main Interface */}
          <div className="grid grid-cols-12 gap-6 max-w-[1800px] mx-auto">
            
            {/* Left Panel - Main Workspace */}
            <div className="col-span-12 lg:col-span-8">
              <MainWorkspace 
                activeMode={activeMode}
                setActiveMode={setActiveMode}
                onCreateArchive={handleCreateArchive}
                onLoadArchive={handleLoadArchive}
                onExtractArchive={handleExtractArchive}
                selectedArchive={selectedArchive}
                setSelectedArchive={setSelectedArchive}
                isProcessing={isProcessing}
                processingType={processingType}
                progress={progress}
                speed={speed}
                finalMessage={finalMessage}
              />
            </div>

            {/* Right Panel - Controls & Metrics */}
            <div className="col-span-12 lg:col-span-4 space-y-6">
              
              {/* Metrics Panel */}
              <TaskProgress 
                progress={progress}
                speed={speed}
                isCreating={processingType === 'create'}
                isExtracting={processingType === 'extract'}
                processedFiles={processedFiles}
                totalFiles={totalFiles}
                processedBytes={processedBytes}
                totalBytes={totalBytes}
                completedShards={completedShards}
                totalShards={totalShards}
                elapsedTime={elapsedTime}
                etaSeconds={etaSeconds}
                compressionRatio={compressionRatio}
              />

              {/* Control Dashboard */}
              <ControlDashboard 
                settings={settings}
                onSettingsChange={setSettings}
                disabled={isProcessing}
              />

              {/* System Status */}
              <SystemStatus 
                archives={archives}
                logs={logs}
                isProcessing={isProcessing}
                onDeleteArchive={handleDeleteArchive}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
    <ResultModal isOpen={isResultModalOpen} onClose={() => setIsResultModalOpen(false)} result={resultData} />
  </>
  );
}