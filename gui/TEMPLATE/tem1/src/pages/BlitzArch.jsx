
import React, { useState, useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Cpu,
  HardDrive
} from 'lucide-react';

import MainWorkspace from '../components/archiver/MainWorkspace';
import ControlDashboard from '../components/archiver/ControlDashboard';
import MetricsPanel from '../components/archiver/MetricsPanel';
import SystemStatus from '../components/archiver/SystemStatus';
import TestPanel from '../components/archiver/TestPanel'; // New import for TestPanel

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

// Проверка доступности Neutralino
const isNeutralinoAvailable = () => {
  return typeof window !== 'undefined' && window.Neutralino;
};

// Мок-функции для разработки без Neutralino
const mockNeutralinoFunctions = {
  execCommand: async (command) => {
    console.log('Mock execCommand:', command);
    return { 
      exitCode: 0, 
      stdOut: 'Mock output for: ' + command,
      stdErr: ''
    };
  },
  getPath: async (pathType) => {
    console.log('Mock getPath:', pathType);
    return pathType === 'documents' ? '/Users/mock/Documents' : './';
  }
};

// Интеграция с Rust-бэкендом
const BlitzArchEngine = {
  async createArchive(files, archiveName, outputDir, compressionLevel) {
    const fileArgs = files.map(f => `"${f}"`).join(' ');
    const command = `bin/blitzarch-engine create --name "${archiveName}" --output "${outputDir}" --files ${fileArgs} --compression ${compressionLevel}`;
    
    console.log("Executing:", command);
    
    try {
      let result;
      if (isNeutralinoAvailable()) {
        result = await window.Neutralino.os.execCommand(command);
      } else {
        result = await mockNeutralinoFunctions.execCommand(command);
      }
      
      if (result.exitCode === 0) {
        console.log('Архив успешно создан:', result.stdOut);
        return { success: true, output: result.stdOut };
      } else {
        console.error('Ошибка при создании архива:', result.stdErr);
        return { success: false, error: result.stdErr };
      }
    } catch (error) {
      console.error('Не удалось выполнить команду создания архива:', error);
      return { success: false, error: error.message };
    }
  },

  async extractArchive(archivePath, destinationPath) {
    const command = `bin/blitzarch-engine extract --archive "${archivePath}" --destination "${destinationPath}"`;
    
    console.log("Executing:", command);
    
    try {
      let result;
      if (isNeutralinoAvailable()) {
        result = await window.Neutralino.os.execCommand(command);
      } else {
        result = await mockNeutralinoFunctions.execCommand(command);
      }
      
      if (result.exitCode === 0) {
        console.log('Архив успешно извлечен:', result.stdOut);
        return { success: true, output: result.stdOut };
      } else {
        console.error('Ошибка при извлечении архива:', result.stdErr);
        return { success: false, error: result.stdErr };
      }
    } catch (error) {
      console.error('Не удалось выполнить команду извлечения:', error);
      return { success: false, error: error.message };
    }
  },

  async listArchiveContents(archivePath) {
    const command = `bin/blitzarch-engine list --archive "${archivePath}"`;
    
    console.log("Executing:", command);
    
    try {
      let result;
      if (isNeutralinoAvailable()) {
        result = await window.Neutralino.os.execCommand(command);
      } else {
        result = await mockNeutralinoFunctions.execCommand(command);
      }
      
      if (result.exitCode === 0) {
        console.log('Содержимое архива:', result.stdOut);
        const fileList = result.stdOut.split('\n').filter(line => line.trim());
        return { success: true, files: fileList };
      } else {
        console.error('Ошибка при просмотре архива:', result.stdErr);
        return { success: false, error: result.stdErr };
      }
    } catch (error) {
      console.error('Не удалось выполнить команду просмотра:', error);
      return { success: false, error: error.message };
    }
  },

  async deleteArchive(archivePath) {
    const command = `bin/blitzarch-engine delete --archive "${archivePath}"`;
    
    console.log("Executing:", command);
    
    try {
      let result;
      if (isNeutralinoAvailable()) {
        result = await window.Neutralino.os.execCommand(command);
      } else {
        result = await mockNeutralinoFunctions.execCommand(command);
      }
      
      if (result.exitCode === 0) {
        console.log('Архив успешно удален:', result.stdOut);
        return { success: true, output: result.stdOut };
      } else {
        console.error('Ошибка при удалении архива:', result.stdErr);
        return { success: false, error: result.stdErr };
      }
    } catch (error) {
      console.error('Не удалось выполнить команду удаления:', error);
      return { success: false, error: error.message };
    }
  }
};


export default function BlitzArch() {
  const [activeMode, setActiveMode] = useState('create');
  const [isProcessing, setIsProcessing] = useState(false);
  const [processingType, setProcessingType] = useState(null);
  const [progress, setProgress] = useState(0);
  const [speed, setSpeed] = useState(0);
  const [selectedArchive, setSelectedArchive] = useState(null);
  const [archives, setArchives] = useState([]);
  const [logs, setLogs] = useState([]);
  const [settings, setSettings] = useState({
    preset: 'balanced',
    compressionLevel: 6,
    password: '',
    useEncryption: false,
    threads: 0,
    codecThreads: 0
  });

  const addLog = (message, type = 'info') => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs(prev => [...prev.slice(-9), { message, type, timestamp }]);
  };

  const handleCreateArchive = async (filePaths) => {
    if (!filePaths || filePaths.length === 0) {
      addLog('No files selected for archiving', 'warning');
      return;
    }
    
    setIsProcessing(true);
    setProcessingType('create');
    setProgress(0);
    setSpeed(0);
    
    const archiveName = `archive_${Date.now()}.blz`;
    
    // Получаем путь к папке Documents
    let outputDir;
    try {
      if (isNeutralinoAvailable()) {
        outputDir = await window.Neutralino.os.getPath('documents');
      } else {
        outputDir = await mockNeutralinoFunctions.getPath('documents');
      }
    } catch (error) {
      outputDir = './'; // fallback
    }
    
    addLog(`Starting archive creation: ${archiveName}`, 'info');
    addLog(`Files to archive: ${filePaths.length} items`, 'info');
    
    try {
      const result = await BlitzArchEngine.createArchive(
        filePaths,
        archiveName,
        outputDir,
        settings.compressionLevel
      );
      
      if (result.success) {
        addLog(`Archive created successfully: ${archiveName}`, 'success');
        setArchives(prev => [...prev, {
          name: archiveName,
          path: `${outputDir}/${archiveName}`,
          size: 0,
          created: new Date(),
          compression_level: settings.compressionLevel,
          preset: settings.preset
        }]);
      } else {
        addLog(`Failed to create archive: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`Error creating archive: ${error.message}`, 'error');
    } finally {
      setIsProcessing(false);
      setProcessingType(null);
      setProgress(100);
    }
  };

  const handleLoadArchive = async (archivePath) => {
    if (!archivePath) {
      addLog('No archive selected', 'warning');
      return;
    }
    
    addLog(`Loading archive: ${archivePath}`, 'info');
    
    try {
      const result = await BlitzArchEngine.listArchiveContents(archivePath);
      
      if (result.success) {
        const mockArchive = {
          name: archivePath.split('/').pop(),
          path: archivePath,
          files: result.files.map((fileName, index) => ({
            name: fileName,
            path: fileName,
            size: Math.floor(Math.random() * 5000000) + 1000,
            crc32: Math.floor(Math.random() * 0xFFFFFFFF).toString(16).toUpperCase().padStart(8, '0'),
            crc_ok: Math.random() > 0.05,
            is_dir: false
          })),
          encrypted: false
        };
        
        setSelectedArchive(mockArchive);
        setActiveMode('browse');
        addLog(`Archive loaded: ${result.files.length} files found`, 'success');
      } else {
        addLog(`Failed to load archive: ${result.error}`, 'error');
      }
    } catch (error) {
      addLog(`Error loading archive: ${error.message}`, 'error');
    }
  };

  const handleExtractArchive = async (selectedFiles = []) => {
    if (!selectedArchive) {
      addLog('No archive selected for extraction', 'warning');
      return;
    }
    
    setIsProcessing(true);
    setProcessingType('extract');
    setProgress(0);
    setSpeed(0);
    
    let destinationPath;
    try {
      if (isNeutralinoAvailable()) {
        destinationPath = await window.Neutralino.os.getPath('documents');
      } else {
        destinationPath = await mockNeutralinoFunctions.getPath('documents');
      }
    } catch (error) {
      destinationPath = './'; // fallback
    }
    
    addLog(`Starting extraction: ${selectedArchive.name}`, 'info');
    
    try {
      const result = await BlitzArchEngine.extractArchive(
        selectedArchive.path,
        destinationPath
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
      const result = await BlitzArchEngine.deleteArchive(archivePath);
      
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

  // Симуляция прогресса для визуального отображения
  useEffect(() => {
    if (!isProcessing) return;
    
    const interval = setInterval(() => {
      setProgress(prev => {
        if (prev >= 95) return prev;
        return prev + Math.random() * 8 + 2;
      });
      
      setSpeed(prev => Math.max(10, prev + (Math.random() - 0.5) * 20));
    }, 200);

    return () => clearInterval(interval);
  }, [isProcessing]);

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

        {/* Test Panel */}
        <TestPanel />

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
              />
            </div>

            {/* Right Panel - Controls & Metrics */}
            <div className="col-span-12 lg:col-span-4 space-y-6">
              
              {/* Metrics Panel */}
              <MetricsPanel 
                speed={speed}
                progress={progress}
                isActive={isProcessing}
                type={processingType}
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
