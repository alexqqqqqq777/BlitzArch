import React, { useState, useCallback } from 'react';
import { motion } from 'framer-motion';
import { Upload, Archive, Plus, Sparkles } from 'lucide-react';

const ACCENT_COLORS = {
  emerald: {
    border: 'border-emerald-400',
    bg: 'bg-emerald-500/10',
    button: 'bg-emerald-500',
    text: 'text-emerald-400',
    shadow: 'shadow-emerald-500/50',
    particles: 'bg-emerald-400'
  },
  amber: {
    border: 'border-amber-400',
    bg: 'bg-amber-500/10',
    button: 'bg-amber-500',
    text: 'text-amber-400',
    shadow: 'shadow-amber-500/50',
    particles: 'bg-amber-400'
  },
  violet: {
    border: 'border-violet-400',
    bg: 'bg-violet-500/10',
    button: 'bg-violet-500',
    text: 'text-violet-400',
    shadow: 'shadow-violet-500/50',
    particles: 'bg-violet-400'
  }
};

export default function DropZone({ 
  onFilesDropped, 
  isProcessing, 
  acceptArchives = false,
  title,
  subtitle,
  accentColor = 'emerald'
}) {
  const [isDragOver, setIsDragOver] = useState(false);
  const colors = ACCENT_COLORS[accentColor];

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
  }, []);

  const handleDrop = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);

    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      onFilesDropped(files);
    }
  }, [onFilesDropped]);

  const handleFileSelect = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0) {
      onFilesDropped(files);
    }
  };

  return (
    <div
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
      className={`relative min-h-[300px] rounded-2xl border-2 border-dashed transition-all duration-300 ${
        isDragOver
          ? `${colors.border} ${colors.bg}`
          : isProcessing
          ? 'border-orange-400 bg-orange-500/10'
          : 'border-slate-600 bg-slate-800/20'
      }`}
    >
      <div className="absolute inset-0 flex flex-col items-center justify-center p-8">
        
        <motion.div
          animate={{
            scale: isDragOver ? 1.1 : isProcessing ? [1, 1.05, 1] : 1,
            rotate: isProcessing ? [0, 360] : 0
          }}
          transition={{
            scale: { duration: 0.2 },
            rotate: { duration: 2, repeat: isProcessing ? Infinity : 0, ease: "linear" }
          }}
          className={`w-20 h-20 rounded-full flex items-center justify-center mb-6 ${
            isDragOver
              ? `${colors.button} shadow-lg ${colors.shadow}`
              : isProcessing
              ? 'bg-gradient-to-r from-orange-500 to-red-500 shadow-lg shadow-orange-500/50'
              : 'bg-slate-700'
          }`}
        >
          {isProcessing ? (
            <Sparkles className="w-10 h-10 text-white" />
          ) : acceptArchives ? (
            <Archive className={`w-10 h-10 ${isDragOver ? 'text-white' : colors.text}`} />
          ) : (
            <Upload className={`w-10 h-10 ${isDragOver ? 'text-white' : colors.text}`} />
          )}
        </motion.div>

        <h3 className={`text-2xl font-bold mb-2 transition-colors ${
          isDragOver ? colors.text : isProcessing ? 'text-orange-400' : 'text-white'
        }`}>
          {isProcessing ? 'Обработка...' : title}
        </h3>

        <p className={`text-center mb-6 transition-colors ${
          isDragOver ? colors.text.replace('400', '300') : isProcessing ? 'text-orange-300' : 'text-slate-400'
        }`}>
          {isProcessing ? 'Пожалуйста, подождите' : subtitle}
        </p>

        {!isProcessing && (
          <div className="flex flex-col sm:flex-row gap-4">
            <input
              type="file"
              multiple={!acceptArchives}
              accept={acceptArchives ? '.blz,.zip,.rar,.7z' : '*/*'}
              onChange={handleFileSelect}
              className="hidden"
              id="file-input"
            />
            
            <label
              htmlFor="file-input"
              className={`px-8 py-4 rounded-xl font-medium cursor-pointer transition-all duration-200 ${
                isDragOver
                  ? `${colors.button} text-white shadow-lg ${colors.shadow}`
                  : 'bg-slate-800/50 text-white hover:bg-slate-700/70 border border-slate-600'
              }`}
            >
              <Plus className="w-5 h-5 inline mr-2" />
              Выбрать файлы
            </label>
          </div>
        )}

        {/* Animated particles */}
        {isDragOver && (
          <div className="absolute inset-0 pointer-events-none">
            {[...Array(12)].map((_, i) => (
              <motion.div
                key={i}
                initial={{ opacity: 0, scale: 0 }}
                animate={{ 
                  opacity: [0, 1, 0],
                  scale: [0, 1, 0],
                  x: [0, (i % 4 - 2) * 150],
                  y: [0, (Math.floor(i / 4) - 1) * 100]
                }}
                transition={{
                  duration: 2,
                  repeat: Infinity,
                  delay: i * 0.15
                }}
                className={`absolute top-1/2 left-1/2 w-2 h-2 ${colors.particles} rounded-full`}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}