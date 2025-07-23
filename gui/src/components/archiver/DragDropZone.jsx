import React, { useState, useCallback } from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { motion, AnimatePresence } from 'framer-motion';
import { Upload, Archive, Sparkles, Zap } from 'lucide-react';

export default function DragDropZone({ 
  onFilesDropped, 
  onArchiveDropped, 
  isActive, 
  title, 
  subtitle,
  acceptedTypes = []
}) {
  const [isDragOver, setIsDragOver] = useState(false);
  const [dragCount, setDragCount] = useState(0);

  const handleDragEnter = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCount(prev => prev + 1);
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCount(prev => {
      const newCount = prev - 1;
      if (newCount <= 0) {
        setIsDragOver(false);
      }
      return newCount;
    });
  }, []);

  const handleDragOver = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    setDragCount(0);

    const files = Array.from(e.dataTransfer.files);
    
    if (acceptedTypes.length > 0) {
      const archiveFiles = files.filter(file => 
        acceptedTypes.some(type => file.name.toLowerCase().endsWith(type))
      );
      if (archiveFiles.length > 0 && onArchiveDropped) {
        onArchiveDropped(archiveFiles[0]);
      }
    } else {
      if (onFilesDropped) {
        onFilesDropped(files);
      }
    }
  }, [acceptedTypes, onFilesDropped, onArchiveDropped]);

  const handleFileInput = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0 && onFilesDropped) {
      onFilesDropped(files);
    }
  };

  return (
    <Card className={`relative overflow-hidden transition-all duration-300 ${
      isDragOver 
        ? 'border-2 border-cyan-400 bg-cyan-500/10 shadow-lg shadow-cyan-500/20' 
        : 'border-2 border-dashed border-slate-600 bg-slate-800/30'
    } ${isActive ? 'animate-pulse border-yellow-400' : ''}`}>
      <CardContent 
        className="p-8 min-h-[200px] flex flex-col items-center justify-center relative"
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDragOver={handleDragOver}
        onDrop={handleDrop}
      >
        {/* Animated background effects */}
        <div className="absolute inset-0 opacity-20">
          <div className={`absolute top-0 left-0 w-full h-full bg-gradient-to-r transition-all duration-500 ${
            isDragOver ? 'from-cyan-400 to-blue-500' : 'from-slate-700 to-slate-800'
          }`} />
        </div>

        <motion.div
          animate={{
            scale: isDragOver ? 1.1 : 1,
            rotate: isDragOver ? 360 : 0
          }}
          transition={{ duration: 0.3 }}
          className="relative z-10 mb-4"
        >
          <div className={`w-16 h-16 rounded-full flex items-center justify-center transition-all duration-300 ${
            isDragOver 
              ? 'bg-cyan-500 shadow-lg shadow-cyan-500/50' 
              : 'bg-slate-700'
          }`}>
            {acceptedTypes.length > 0 ? (
              <Archive className={`w-8 h-8 ${isDragOver ? 'text-black' : 'text-cyan-400'}`} />
            ) : (
              <Upload className={`w-8 h-8 ${isDragOver ? 'text-black' : 'text-cyan-400'}`} />
            )}
          </div>
        </motion.div>

        <h3 className={`text-xl font-bold mb-2 transition-colors duration-300 ${
          isDragOver ? 'text-cyan-300' : 'text-slate-200'
        }`}>
          {title}
        </h3>
        
        <p className={`text-sm text-center mb-4 transition-colors duration-300 ${
          isDragOver ? 'text-cyan-400' : 'text-slate-400'
        }`}>
          {subtitle}
        </p>

        {/* File input for click */}
        <input
          type="file"
          multiple={acceptedTypes.length === 0}
          accept={acceptedTypes.join(',')}
          onChange={handleFileInput}
          className="hidden"
          id="file-input"
        />
        
        <label 
          htmlFor="file-input"
          className={`px-6 py-3 rounded-lg cursor-pointer transition-all duration-300 font-medium ${
            isDragOver 
              ? 'bg-cyan-500 text-black shadow-lg shadow-cyan-500/50' 
              : 'bg-slate-700 text-cyan-400 hover:bg-slate-600'
          }`}
        >
          Или выберите файлы
        </label>

        {/* Sparkle effects */}
        <AnimatePresence>
          {isDragOver && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute inset-0 pointer-events-none"
            >
              {[...Array(6)].map((_, i) => (
                <motion.div
                  key={i}
                  initial={{ scale: 0, rotate: 0 }}
                  animate={{ 
                    scale: [0, 1, 0], 
                    rotate: [0, 180, 360],
                    x: [0, (i % 2 ? 50 : -50), 0],
                    y: [0, (i % 3 ? 30 : -30), 0]
                  }}
                  transition={{
                    duration: 2,
                    repeat: Infinity,
                    delay: i * 0.2
                  }}
                  className="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2"
                >
                  <Sparkles className="w-4 h-4 text-yellow-400" />
                </motion.div>
              ))}
            </motion.div>
          )}
        </AnimatePresence>
      </CardContent>
    </Card>
  );
}