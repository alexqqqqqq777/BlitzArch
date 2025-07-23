import React, { useState, useCallback, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { Upload, Archive, FolderOpen, Plus } from 'lucide-react';

const COLOR_SCHEMES = {
  teal: {
    border: 'border-teal-400',
    bg: 'bg-teal-500/10',
    button: 'bg-gradient-to-r from-teal-500 to-cyan-500',
    text: 'text-teal-400',
    glow: 'shadow-teal-500/30',
    ring: 'ring-teal-400/20'
  },
  emerald: {
    border: 'border-emerald-400',
    bg: 'bg-emerald-500/10',
    button: 'bg-gradient-to-r from-emerald-500 to-teal-500',
    text: 'text-emerald-400',
    glow: 'shadow-emerald-500/30',
    ring: 'ring-emerald-400/20'
  },
  cyan: {
    border: 'border-cyan-400',
    bg: 'bg-cyan-500/10',
    button: 'bg-gradient-to-r from-cyan-500 to-blue-500',
    text: 'text-cyan-400',
    glow: 'shadow-cyan-500/30',
    ring: 'ring-cyan-400/20'
  }
};

export default function WorkspaceDropZone({ 
  onFilesSelected, 
  title,
  subtitle,
  acceptMultiple = false,
  acceptArchives = false,
  color = 'teal'
}) {
  const [isDragOver, setIsDragOver] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const scheme = COLOR_SCHEMES[color];

  // Tauri drag & drop события для получения реальных путей
  useEffect(() => {
    console.log('🔧 Setting up Tauri drag & drop listeners');
    
    let unlisten;
    
    const setupTauriDragDrop = async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        
        // Слушаем Tauri drag & drop события
        unlisten = await listen('tauri://drag-drop', (event) => {
          console.log('🏗️ TAURI DRAG DROP EVENT:', event.payload);
          
          if (event.payload && event.payload.paths && event.payload.paths.length > 0) {
            const filePaths = event.payload.paths;
            console.log('📁 Real file paths from Tauri:', filePaths);
            
            // Фильтруем файлы по типу (архивы или обычные файлы)
            let filteredPaths = filePaths;
            
            if (acceptArchives) {
              // Для explorer - только архивы
              const archiveExtensions = ['.blz', '.zip', '.rar', '.7z', '.tar', '.gz'];
              filteredPaths = filePaths.filter(path => 
                archiveExtensions.some(ext => path.toLowerCase().endsWith(ext))
              );
              console.log('📁 Filtered archive paths:', filteredPaths);
            }
            
            if (filteredPaths.length === 0) {
              console.warn('⚠️ No valid files found for this drop zone');
              return;
            }
            
            // Создаем объекты файлов с реальными путями
            const filesWithPaths = filteredPaths.map(path => ({
              name: path.split('/').pop() || path.split('\\').pop(),
              path: path,
              size: 0 // Размер неизвестен
            }));
            
            console.log('✅ Files with real paths:', filesWithPaths);
            onFilesSelected(filesWithPaths);
          }
        });
        
        console.log('✅ Tauri drag & drop listener set up');
      } catch (error) {
        console.warn('⚠️ Tauri drag & drop not available:', error);
        console.log('🌐 Falling back to standard drag & drop');
        
        // Fallback к стандартным событиям
        const handleGlobalDragOver = (e) => {
          console.log('🌍 FALLBACK DRAG OVER');
          e.preventDefault();
        };
        
        const handleGlobalDrop = (e) => {
          console.log('🌍 FALLBACK DROP');
          e.preventDefault();
        };
        
        document.addEventListener('dragover', handleGlobalDragOver);
        document.addEventListener('drop', handleGlobalDrop);
        
        return () => {
          document.removeEventListener('dragover', handleGlobalDragOver);
          document.removeEventListener('drop', handleGlobalDrop);
        };
      }
    };
    
    setupTauriDragDrop();
    
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [onFilesSelected]);

  // Упрощенные обработчики drag-and-drop с отладкой
  const handleDragEnter = useCallback((e) => {
    console.log('🎯 COMPONENT DRAG ENTER!');
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e) => {
    console.log('🎯 COMPONENT DRAG LEAVE!');
    e.preventDefault();
    e.stopPropagation();
    if (!e.currentTarget.contains(e.relatedTarget)) {
      setIsDragOver(false);
    }
  }, []);

  const handleDragOver = useCallback((e) => {
    console.log('🎯 COMPONENT DRAG OVER!');
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
  }, []);

  const handleDrop = useCallback(async (e) => {
    console.log('🎯 DROP EVENT TRIGGERED!', e);
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    
    console.log('📋 DataTransfer:', e.dataTransfer);
    console.log('📋 DataTransfer.files:', e.dataTransfer.files);
    
    // Обрабатываем перетащенные файлы
    const files = Array.from(e.dataTransfer.files);
    console.log('📁 Files array:', files);
    console.log('📁 Files length:', files.length);
    
    if (files.length > 0) {
      console.log('✅ Processing dropped files:', files.map(f => f.name));
      
      // Простая передача файлов без сложной логики
      console.log('📁 Calling onFilesSelected with files:', files);
      onFilesSelected(files);
    } else {
      console.log('❌ No files found in drop event');
      
      // Попробуем альтернативные способы получения файлов
      const items = Array.from(e.dataTransfer.items);
      console.log('📋 DataTransfer.items:', items);
      
      if (items.length > 0) {
        const fileItems = items.filter(item => item.kind === 'file');
        console.log('📁 File items:', fileItems);
        
        const fileObjects = fileItems.map(item => item.getAsFile()).filter(Boolean);
        console.log('📁 File objects from items:', fileObjects);
        
        if (fileObjects.length > 0) {
          console.log('✅ Using files from items');
          onFilesSelected(fileObjects);
        }
      }
    }
  }, [onFilesSelected]);

  // Tauri file dialog for getting real file paths
  const handleSelectFiles = async () => {
    if (isProcessing) return;
    
    try {
      // Use Tauri dialog API
      const { open } = await import('@tauri-apps/plugin-dialog');
      
      console.log('🔍 Using Tauri file dialog');
      
      let filters = [];
      if (acceptArchives) {
        filters = [
          {
            name: 'Archive files',
            extensions: ['blz', 'zip', 'rar', '7z', 'tar', 'gz']
          }
        ];
      }
      
      const selected = await open({
        multiple: acceptMultiple,
        filters: filters.length > 0 ? filters : undefined,
        directory: false
      });
      
      if (selected) {
        const filePaths = Array.isArray(selected) ? selected : [selected];
        
        // Create file objects with real paths
        const filesWithPaths = filePaths.map(path => ({
          name: path.split('/').pop() || path.split('\\').pop(),
          path: path,
          size: 0 // We don't have size info from Tauri dialog
        }));
        
        console.log('📁 Selected files with real paths:', filesWithPaths);
        onFilesSelected(filesWithPaths);
      }
    } catch (error) {
      console.error('❌ Error selecting files:', error);
      
      // Fallback to regular file input if Tauri dialog fails
      console.log('🔍 Falling back to regular file input');
      
      const input = document.createElement('input');
      input.type = 'file';
      input.multiple = acceptMultiple;
      
      if (acceptArchives) {
        input.accept = '.blz,.zip,.rar,.7z,.tar,.gz';
      }
      
      input.onchange = (e) => {
        const files = Array.from(e.target.files);
        if (files.length > 0) {
          console.log('📁 Selected files (fallback):', files.map(f => f.name));
          onFilesSelected(files);
        }
      };
      
      input.click();
    }
  };

  const handleClick = useCallback((e) => {
    // Only trigger file selection if not dragging
    if (!isDragOver) {
      console.log('🖱️ Click event triggered');
      handleSelectFiles();
    }
  }, [isDragOver, handleSelectFiles]);

  const handleMouseEnter = useCallback(() => {
    console.log('🖱️ MOUSE ENTER DROP ZONE - Events are working!');
  }, []);

  const handleMouseLeave = useCallback(() => {
    console.log('🖱️ MOUSE LEAVE DROP ZONE');
  }, []);

  return (
    <div
      data-dropzone="true"
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      className={`relative min-h-[400px] rounded-2xl border-2 border-dashed transition-all duration-300 cursor-pointer z-10 ${
        isDragOver
          ? `${scheme.border} ${scheme.bg} ring-4 ${scheme.ring}`
          : isProcessing
          ? 'border-orange-400 bg-orange-500/10'
          : 'border-neutral-600 bg-neutral-800/20'
      }`}
      onClick={handleClick}
      style={{
        pointerEvents: 'auto',
        position: 'relative',
        zIndex: 10
      }}
    >
      
      {/* Background Pattern */}
      <div className="absolute inset-0 opacity-5">
        <div 
          className="w-full h-full rounded-2xl" 
          style={{
            backgroundImage: `radial-gradient(circle at 25% 25%, rgba(115, 115, 115, 0.2) 0%, transparent 50%), 
                             radial-gradient(circle at 75% 75%, rgba(115, 115, 115, 0.2) 0%, transparent 50%)`
          }}
        />
      </div>

      <div className="relative z-10 flex flex-col items-center justify-center h-full p-12">
        
        {/* Icon */}
        <motion.div
          animate={{
            scale: isDragOver ? 1.2 : isProcessing ? [1, 1.1, 1] : 1,
            rotate: isProcessing ? [0, 360] : 0
          }}
          transition={{ 
            rotate: { duration: 2, repeat: isProcessing ? Infinity : 0, ease: "linear" }
          }}
          className={`w-24 h-24 rounded-2xl flex items-center justify-center mb-8 transition-all duration-300 ${
            isDragOver
              ? `${scheme.button} shadow-2xl ${scheme.glow}`
              : isProcessing
              ? 'bg-gradient-to-r from-orange-500 to-amber-500'
              : 'bg-neutral-700/50 border border-neutral-600'
          }`}
        >
          {isProcessing ? (
            <FolderOpen className="w-12 h-12 text-white animate-pulse" />
          ) : acceptArchives ? (
            <Archive className={`w-12 h-12 ${isDragOver ? 'text-white' : scheme.text}`} />
          ) : (
            <Upload className={`w-12 h-12 ${isDragOver ? 'text-white' : scheme.text}`} />
          )}
        </motion.div>

        {/* Title */}
        <h3 className={`text-2xl font-bold mb-3 text-center transition-colors duration-300 ${
          isDragOver ? 'text-white' : isProcessing ? 'text-orange-400' : 'text-white'
        }`}>
          {isProcessing ? 'Opening Dialog...' : isDragOver ? 'Drop Files Here!' : title}
        </h3>

        {/* Subtitle */}
        <p className={`text-lg text-center mb-8 transition-colors duration-300 ${
          isDragOver ? 'text-white/80' : isProcessing ? 'text-orange-300' : 'text-neutral-400'
        }`}>
          {isProcessing ? 'Please wait...' : isDragOver ? 'Release to select files' : subtitle}
        </p>

        {/* Action Button */}
        {!isProcessing && (
          <Button
            size="lg"
            onClick={(e) => {
              e.stopPropagation();
              handleSelectFiles();
            }}
            className={`transition-all duration-300 ${
              isDragOver
                ? `${scheme.button} text-white shadow-lg ${scheme.glow}`
                : 'bg-neutral-700 text-neutral-300 hover:bg-neutral-600'
            }`}
          >
            <Plus className="w-5 h-5 mr-3" />
            {acceptArchives ? 'Select Archive' : 'Select Files'}
          </Button>
        )}

        {/* Floating particles animation */}
        {isDragOver && (
          <div className="absolute inset-0 pointer-events-none overflow-hidden rounded-2xl">
            {[...Array(8)].map((_, i) => (
              <motion.div
                key={i}
                initial={{ opacity: 0, scale: 0 }}
                animate={{ 
                  opacity: [0, 1, 0],
                  scale: [0, 1, 0],
                  x: [0, (i % 4 - 1.5) * 200],
                  y: [0, (Math.floor(i / 4) - 0.5) * 200]
                }}
                transition={{
                  duration: 2,
                  repeat: Infinity,
                  delay: i * 0.2
                }}
                className={`absolute top-1/2 left-1/2 w-2 h-2 ${scheme.text.replace('text-', 'bg-')} rounded-full`}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}