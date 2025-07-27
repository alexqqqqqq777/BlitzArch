import React, { useState, useCallback } from 'react';
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

  // Правильные обработчики drag-and-drop
  const handleDragEnter = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    if (!e.currentTarget.contains(e.relatedTarget)) {
      setIsDragOver(false);
    }
  }, []);

  const handleDragOver = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'copy';
  }, []);

  const handleDrop = useCallback(async (e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    
    // Открываем диалог для получения полных путей
    await handleSelectFiles();
  }, []);

  // Нативный диалог выбора файлов
  const handleSelectFiles = async () => {
    if (isProcessing) return;
    
    setIsProcessing(true);
    
    try {
      let selectedPaths = [];

      if (typeof window !== 'undefined' && window.Neutralino) {
        if (acceptArchives) {
          const result = await window.Neutralino.os.showOpenDialog('Select Archive', {
            multiSelections: acceptMultiple,
            filters: [
              { name: 'BlitzArch Archives', extensions: ['blz'] },
              { name: 'All Archives', extensions: ['zip', 'rar', '7z', 'tar', 'gz'] }
            ]
          });
          selectedPaths = Array.isArray(result) ? result : [result];
        } else {
          const result = await window.Neutralino.os.showOpenDialog('Select Files and Folders', {
            multiSelections: true,
          });
          selectedPaths = Array.isArray(result) ? result : [result];
        }
      } else {
        // Мок-данные для разработки
        selectedPaths = acceptArchives 
          ? ['/mock/path/archive.blz']
          : ['/mock/path/file1.txt', '/mock/path/file2.jpg', '/mock/path/folder/'];
      }

      if (selectedPaths.length > 0 && selectedPaths[0] && selectedPaths[0] !== '') {
        onFilesSelected(selectedPaths);
      }
    } catch (error) {
      console.error('Error selecting files:', error);
    } finally {
      setIsProcessing(false);
    }
  };

  return (
    <div
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      className={`relative min-h-[400px] rounded-2xl border-2 border-dashed transition-all duration-300 cursor-pointer ${
        isDragOver
          ? `${scheme.border} ${scheme.bg} ring-4 ${scheme.ring}`
          : isProcessing
          ? 'border-orange-400 bg-orange-500/10'
          : 'border-neutral-600 bg-neutral-800/20'
      }`}
      onClick={handleSelectFiles}
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