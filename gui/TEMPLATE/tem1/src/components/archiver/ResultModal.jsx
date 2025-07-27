import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { 
  CheckCircle, 
  XCircle, 
  AlertTriangle,
  FileCheck,
  Archive,
  Download,
  Upload,
  Clock,
  HardDrive,
  Shield,
  X,
  Copy,
  ExternalLink
} from 'lucide-react';

export default function ResultModal({ isOpen, onClose, result }) {
  if (!isOpen || !result) return null;

  const getResultInfo = () => {
    switch (result.type) {
      case 'create_success':
        return {
          icon: CheckCircle,
          title: 'Archive Created Successfully',
          color: 'text-emerald-400',
          bgColor: 'bg-emerald-500/10',
          borderColor: 'border-emerald-500/30',
          gradient: 'from-emerald-500 to-teal-500'
        };
      case 'extract_success':
        return {
          icon: Download,
          title: 'Archive Extracted Successfully', 
          color: 'text-violet-400',
          bgColor: 'bg-violet-500/10',
          borderColor: 'border-violet-500/30',
          gradient: 'from-violet-500 to-purple-500'
        };
      case 'create_error':
        return {
          icon: XCircle,
          title: 'Archive Creation Failed',
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          gradient: 'from-red-500 to-rose-500'
        };
      case 'extract_error':
        return {
          icon: XCircle,
          title: 'Extraction Failed',
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          gradient: 'from-red-500 to-rose-500'
        };
      case 'password_error':
        return {
          icon: Shield,
          title: 'Invalid Password',
          color: 'text-orange-400',
          bgColor: 'bg-orange-500/10',
          borderColor: 'border-orange-500/30',
          gradient: 'from-orange-500 to-amber-500'
        };
      default:
        return {
          icon: AlertTriangle,
          title: 'Unknown Result',
          color: 'text-yellow-400',
          bgColor: 'bg-yellow-500/10',
          borderColor: 'border-yellow-500/30',
          gradient: 'from-yellow-500 to-orange-500'
        };
    }
  };

  const info = getResultInfo();
  const Icon = info.icon;
  const isSuccess = result.type.includes('success');
  const isError = result.type.includes('error');

  const formatSize = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const formatDuration = (seconds) => {
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds}s`;
  };

  const copyToClipboard = (text) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.9, y: 20 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.9, y: -20 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className={`relative w-full max-w-lg mx-4 bg-neutral-900 rounded-2xl border ${info.borderColor} shadow-2xl`}
          onClick={(e) => e.stopPropagation()}
        >
          
          {/* Header */}
          <div className={`p-6 pb-0`}>
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-4">
                <motion.div
                  initial={{ scale: 0 }}
                  animate={{ scale: 1 }}
                  transition={{ delay: 0.2 }}
                  className={`w-12 h-12 rounded-xl flex items-center justify-center bg-gradient-to-r ${info.gradient}`}
                >
                  <Icon className="w-6 h-6 text-white" />
                </motion.div>
                <div>
                  <h2 className="text-xl font-bold text-white">{info.title}</h2>
                  <p className="text-neutral-400 text-sm">
                    {new Date().toLocaleString()}
                  </p>
                </div>
              </div>
              
              <Button
                variant="ghost"
                size="icon"
                onClick={onClose}
                className="text-neutral-400 hover:text-white hover:bg-neutral-800"
              >
                <X className="w-5 h-5" />
              </Button>
            </div>

            {/* Status Message */}
            <div className={`p-4 rounded-xl ${info.bgColor} border ${info.borderColor} mb-6`}>
              <p className={`text-sm font-medium ${info.color}`}>
                {result.message || 'Operation completed'}
              </p>
            </div>
          </div>

          {/* Details */}
          <div className="px-6 pb-6">
            
            {/* Success Details */}
            {isSuccess && (
              <div className="grid grid-cols-2 gap-4 mb-6">
                <div className="p-3 rounded-lg bg-neutral-800/50">
                  <div className="flex items-center gap-2 mb-2">
                    <Archive className="w-4 h-4 text-cyan-400" />
                    <span className="text-sm font-medium text-neutral-300">Archive</span>
                  </div>
                  <div className="text-white font-medium truncate">
                    {result.archiveName || 'archive.blz'}
                  </div>
                  <div className="text-xs text-neutral-400">
                    {formatSize(result.archiveSize)}
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-neutral-800/50">
                  <div className="flex items-center gap-2 mb-2">
                    <Clock className="w-4 h-4 text-emerald-400" />
                    <span className="text-sm font-medium text-neutral-300">Duration</span>
                  </div>
                  <div className="text-white font-medium">
                    {formatDuration(result.duration || 3)}
                  </div>
                  <div className="text-xs text-neutral-400">
                    {(result.speed || 45).toFixed(1)} MB/s avg
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-neutral-800/50">
                  <div className="flex items-center gap-2 mb-2">
                    <FileCheck className="w-4 h-4 text-violet-400" />
                    <span className="text-sm font-medium text-neutral-300">Files</span>
                  </div>
                  <div className="text-white font-medium">
                    {result.filesCount || 12} files
                  </div>
                  <div className="text-xs text-neutral-400">
                    {result.foldersCount || 3} folders
                  </div>
                </div>

                <div className="p-3 rounded-lg bg-neutral-800/50">
                  <div className="flex items-center gap-2 mb-2">
                    <HardDrive className="w-4 h-4 text-amber-400" />
                    <span className="text-sm font-medium text-neutral-300">Compression</span>
                  </div>
                  <div className="text-white font-medium">
                    {result.compressionRatio || '68'}%
                  </div>
                  <div className="text-xs text-neutral-400">
                    Level {result.compressionLevel || 9}
                  </div>
                </div>
              </div>
            )}

            {/* Error Details */}
            {isError && (
              <div className="mb-6">
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700">
                  <h4 className="text-sm font-medium text-neutral-300 mb-2">Error Details</h4>
                  <div className="text-red-400 text-sm font-mono bg-neutral-900/50 p-3 rounded border">
                    {result.error || 'An unknown error occurred'}
                  </div>
                  
                  {result.type === 'password_error' && (
                    <div className="mt-3 p-3 rounded bg-orange-500/10 border border-orange-500/20">
                      <div className="flex items-center gap-2 text-orange-400 text-sm">
                        <Shield className="w-4 h-4" />
                        <span>The archive is encrypted. Please check your password and try again.</span>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* File Path */}
            {result.outputPath && (
              <div className="mb-6">
                <h4 className="text-sm font-medium text-neutral-300 mb-2">Output Location</h4>
                <div className="flex items-center gap-2 p-3 rounded-lg bg-neutral-800/30 border border-neutral-700">
                  <span className="text-sm text-neutral-400 flex-1 font-mono truncate">
                    {result.outputPath}
                  </span>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => copyToClipboard(result.outputPath)}
                    className="h-8 px-2 text-neutral-400 hover:text-white"
                  >
                    <Copy className="w-4 h-4" />
                  </Button>
                </div>
              </div>
            )}

            {/* Action Buttons */}
            <div className="flex gap-3">
              {isSuccess && result.outputPath && (
                <>
                  <Button
                    variant="outline"
                    onClick={() => {
                      // В Neutralino можно открыть папку
                      if (window.Neutralino) {
                        window.Neutralino.os.showFolderDialog();
                      }
                    }}
                    className="flex-1 border-neutral-600 text-neutral-300 hover:bg-neutral-800"
                  >
                    <ExternalLink className="w-4 h-4 mr-2" />
                    Open Folder
                  </Button>
                  
                  <Button
                    onClick={onClose}
                    className={`flex-1 bg-gradient-to-r ${info.gradient} text-white hover:opacity-90`}
                  >
                    Done
                  </Button>
                </>
              )}

              {isError && (
                <>
                  <Button
                    variant="outline"
                    onClick={onClose}
                    className="flex-1 border-neutral-600 text-neutral-300 hover:bg-neutral-800"
                  >
                    Close
                  </Button>
                  
                  {result.type === 'password_error' && (
                    <Button
                      onClick={() => {
                        onClose();
                        // Здесь можно вызвать повторную попытку
                      }}
                      className="flex-1 bg-gradient-to-r from-orange-500 to-amber-500 text-white hover:opacity-90"
                    >
                      Try Again
                    </Button>
                  )}
                </>
              )}
            </div>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}