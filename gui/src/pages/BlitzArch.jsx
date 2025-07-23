import React, { useState, useEffect, useCallback } from 'react';
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
import tauriBlitzArchEngine from '../lib/tauri-engine.js';
import { invoke } from '@tauri-apps/api/core';
import { determineOutputPath, generateArchiveName, createArchivePath, validateOutputDirectory } from '../lib/path-utils.js';

// Новая стильная иконка с молнией
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
// Умное извлечение с предотвращением дублирования путей
const extractWithSmartPathHandling = async (archive, destinationPath, options = {}) => {
  try {
    // Получаем список файлов в архиве
    const listResult = await tauriBlitzArchEngine.listArchive(archive.path);
    
    if (!listResult.success || !listResult.files || listResult.files.length === 0) {
      console.warn('⚠️ Cannot analyze archive contents, using standard extraction');
      return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
    }
    
    const filePaths = listResult.files;
    console.log('📋 Archive file paths:', filePaths);
    
    // Находим общий корневой путь всех файлов
    const commonRoot = findCommonRootPath(filePaths);
    console.log('🌳 Common root path:', commonRoot);
    
    // Проверяем, нужно ли избегать дублирования путей
    const needsSmartExtraction = commonRoot && (
      destinationPath.includes(commonRoot) || 
      commonRoot.includes(destinationPath.split('/').pop())
    );
    
    if (needsSmartExtraction) {
      console.log('⚠️ Path duplication detected, using smart extraction');
      
      // Создаем временную папку для извлечения
      const tempDir = `${destinationPath}/.blitzarch_temp_${Date.now()}`;
      console.log('📁 Extracting to temp directory:', tempDir);
      
      // Извлекаем во временную папку
      const extractResult = await tauriBlitzArchEngine.extractArchive(
        archive.path, 
        tempDir, 
        options
      );
      
      if (!extractResult.success) {
        return extractResult;
      }
      
      // Перемещаем файлы из временной папки в целевую, избегая дублирования
      console.log('🔄 Moving files to final destination...');
      const moveResult = await moveFilesSmartly(tempDir, destinationPath, commonRoot);
      
      // Очищаем временную папку
      await cleanupTempDirectory(tempDir);
      
      return moveResult;
    } else {
      console.log('✅ No path duplication detected, using standard extraction');
      return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
    }
  } catch (error) {
    console.error('❌ Error in smart extraction:', error);
    // Fallback к стандартному извлечению
    return await tauriBlitzArchEngine.extractArchive(archive.path, destinationPath, options);
  }
};

// Находит общий корневой путь для массива путей
const findCommonRootPath = (paths) => {
  if (!paths || paths.length === 0) return null;
  if (paths.length === 1) {
    // Для одного файла возвращаем его директорию
    const path = paths[0];
    const lastSlash = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return lastSlash > 0 ? path.substring(0, lastSlash) : null;
  }
  
  // Для нескольких файлов находим общий префикс
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
  
  // Обрезаем до последнего слэша
  const lastSlash = Math.max(commonPath.lastIndexOf('/'), commonPath.lastIndexOf('\\'));
  return lastSlash > 0 ? commonPath.substring(0, lastSlash) : null;
};

const createArchiveWithGoldenStandard = async (files, settings) => {
  const {
    compressionLevel = 3,
    password = null,
    bundleSize = 32,
    threads = 0,
    codecThreads = 0, // пока не используется в бэкенде, но оставим для совместимости
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
        outputDir: outputDir
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
        addLog(`🎯 Извлекаем файл: ${dragData.fileName}`, 'info');
        
        // Determine target directory (Downloads folder as fallback)
        const downloadsDir = await invoke('get_downloads_path')
          .catch(() => '/Users/oleksandr/Downloads'); // Fallback
        
        // Extract file using our new command
        const result = await invoke('drag_out_extract', {
  // Передаем обе вариации ключей (snake_case и camelCase) для совместимости
  archive_path: dragData.archivePath,
  archivePath: dragData.archivePath,
  file_path: dragData.filePath,
  filePath: dragData.filePath,
  target_dir: downloadsDir,
  targetDir: downloadsDir,
  password: settings.useEncryption ? settings.password : null
});
        
        if (result.success) {
          addLog(`✅ Файл успешно извлечён: ${result.archive_path}`, 'success');
        } else {
          addLog(`❌ Ошибка извлечения: ${result.error}`, 'error');
        }
        
      } catch (error) {
        console.error('❌ Drag-out error:', error);
        addLog(`❌ Ошибка drag-out: ${error.message}`, 'error');
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
      addLog('Не выбраны файлы для архивации', 'error');
      return;
    }

    startProcessing('create');
    setProgress(0);
    setSpeed(0);
    
    addLog(`🚀 Начинаем создание архива из ${files.length} файлов...`);
    addLog(`📋 Файлы: ${files.map(f => f.name || f).join(', ')}`);
    
    try {
      const result = await createArchiveWithGoldenStandard(files, settings);
      
      if (result.success) {
        addLog(`✅ Архив успешно создан!`, 'success');
        addLog(`📦 Имя архива: ${result.archiveName}`, 'success');
        addLog(`📁 Расположение: ${result.outputDir}`, 'success');
        addLog(`🗂️ Полный путь: ${result.archivePath}`, 'success');
        
        // Добавляем созданный архив в список
        const newArchive = {
          id: Date.now().toString(),
          name: result.archiveName + '.blz',
          path: result.archivePath,
          size: 'Unknown',
          created: new Date().toISOString(),
          files: files.length
        };
        setArchives(prev => [newArchive, ...prev]);
      } else {
        addLog(`❌ Ошибка создания архива: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`💥 Неожиданная ошибка: ${error.message}`, 'error');
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
          files: result.files, // Используем реальные данные, полученные от движка
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
    
    startProcessing('extract');
    setProgress(0);
    setSpeed(0);
    
    // Handle batch extraction of multiple archives
    if (isBatchMode) {
      // --- Batch list sanitation ----------------------------------------------------
      const archiveExts = ['.blz']; // поддерживаемые архивы для batch-mode
      const uniqueByPath = Array.from(new Set(selectedFiles.map(f => (f.path || f).toString())));
      const sanitized = uniqueByPath.filter(p => archiveExts.some(ext => p.toLowerCase().endsWith(ext)));

      if (sanitized.length === 0) {
        addLog('Не найдено валидных архивов для извлечения', 'warning');
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
            addLog(`[${i + 1}/${sanitized.length}] ✅ ${archiveName} extracted successfully`, 'success');
          } else {
            addLog(`[${i + 1}/${sanitized.length}] ❌ Failed to extract ${archiveName}: ${result.error}`, 'error');
          }
        } catch (error) {
          addLog(`[${i + 1}/${sanitized.length}] ❌ Error extracting ${archiveName}: ${error.message}`, 'error');
        }
        
        // Update progress
        // Batch extract manual progress – clamp to 100%
        setProgress(Math.min(Math.round(((i + 1) / sanitized.length) * 100), 100));
      }
      
      addLog(`Batch extraction completed: ${sanitized.length} archives processed`, 'info');
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
        } else {
          addLog(`Failed to extract files from ${archiveName}: ${result.error}`, 'error');
        }
      } catch (error) {
        addLog(`Error extracting files: ${error.message}`, 'error');
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
      } else {
        addLog(`Failed to extract archive: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`Error extracting archive: ${error.message}`, 'error');
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
  );
}