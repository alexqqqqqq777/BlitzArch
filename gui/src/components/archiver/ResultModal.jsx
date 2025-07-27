import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
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
  const [passwordInput, setPasswordInput] = useState('');
  const [isRetrying, setIsRetrying] = useState(false);
  
  if (!isOpen || !result) {
    return null;
  }

  // Unpack statistics from backend, support old keys for backward compatibility
  const stats = result.stats || {};
  const statFiles = stats.files ?? stats.fileCount ?? '—';
  const statDuration = stats.time_sec ?? stats.duration;
  const statRatio = stats.ratio ?? stats.compressionRatio;
  const statSpeed = stats.speed_mb_s ?? stats.speed ?? (stats.total_bytes && stats.time_sec ? (stats.total_bytes / stats.time_sec) / (1024 * 1024) : undefined);
  const statSize = stats.archive_bytes ?? stats.archiveBytes ?? stats.size ?? stats.total_bytes ?? stats.totalBytes;


  const formatRatio = (val) => {
    if (!val && val !== 0) return '—';
    if (typeof val === 'string') return val.includes(':') ? val : `${parseFloat(val).toFixed(2)} : 1`;
    if (typeof val === 'number') {
      // Backend already returns correct compression ratio (original/compressed)
      if (val >= 1000) return `${(val / 1000).toFixed(1)}k : 1`;
      if (val >= 100) return `${val.toFixed(0)} : 1`;
      if (val >= 10) return `${val.toFixed(1)} : 1`;
      return `${val.toFixed(2)} : 1`;
    }
    return '—';
  };
  const formatSpeedMb = (val) => {
    if (!val && val !== 0) return '—';
    if (typeof val === 'string') return val; // already formatted string
    if (typeof val === 'number') {
      if (val >= 1) return val.toFixed(1) + ' MB/s';
      const kb = val * 1024;
      if (kb >= 1) return kb.toFixed(1) + ' KB/s';
      const bytes = kb * 1024;
      if (bytes >= 1) return bytes.toFixed(0) + ' B/s';
      return '< 1 B/s'; // for very small speeds
    }
    return '—';
  };


  

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
          title: 'Archive Creation Error',
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          gradient: 'from-red-500 to-rose-500'
        };
      case 'extract_error':
        return {
          icon: XCircle,
          title: 'Extraction Error',
          color: 'text-red-400',
          bgColor: 'bg-red-500/10',
          borderColor: 'border-red-500/30',
          gradient: 'from-red-500 to-rose-500'
        };
      case 'password_error':
        return {
          icon: Shield,
          title: 'Incorrect Password',
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
        }
    }
  };

  const info = getResultInfo();
  const Icon = info.icon;
  const isSuccess = result.type.includes('success');
  const isError = result.type.includes('error');

  const formatSize = (bytes) => {
    if (bytes === undefined || bytes === null) return '—';
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const formatDuration = (seconds) => {
  if (seconds === undefined || seconds === null || seconds === '') return '—';
  const sec = typeof seconds === 'string' ? parseFloat(seconds) : seconds;
  if (isNaN(sec)) return '—';
  if (sec < 60) return `${sec.toFixed(1)}s`;
  const minutes = Math.floor(sec / 60);
  const remainingSeconds = (sec % 60).toFixed(0);
  return `${minutes}m ${remainingSeconds}s`;
  };

  const formatSpeed = (bytes, seconds) => {
    if (bytes === undefined || bytes === null || seconds === undefined || seconds === null || isNaN(seconds)) return '—';
    const speed = bytes / seconds;
    const k = 1024;
    const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    const i = Math.floor(Math.log(speed) / Math.log(k));
    return parseFloat((speed / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
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
          transition={{ duration: 0.3, ease: 'easeOut' }}
          className={`relative w-full max-w-lg mx-4 bg-neutral-900 rounded-2xl border ${info.borderColor} shadow-2xl`}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div className="p-6 pb-0">
            <div className="flex items-center justify-between mb-6">
              <div className="flex items-center gap-4">
                <motion.div
                  initial={{ scale: 0 }}
                  animate={{ scale: 1 }}
                  transition={{ delay: 0.2 }}
                  className={`p-3 rounded-full ${info.bgColor}`}
                >
                  <Icon className={`w-8 h-8 ${info.color}`} />
                </motion.div>
                <div>
                  <h3 className="text-xl font-bold text-white mb-1">{info.title}</h3>
                  {result.message && (
                    <p className="text-neutral-400 text-sm max-w-xs">{result.message}</p>
                  )}
                </div>
              </div>
              <button onClick={onClose} className="text-neutral-500 hover:text-neutral-300">
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* Integrity Check */}
            {result.integrityOk !== undefined && result.integrityOk !== null && (
              <div className="mb-6 flex items-center gap-3">
                {result.integrityOk ? (
                  <div className="flex items-center gap-2 text-emerald-400 text-sm">
                    <Shield className="w-4 h-4" />
                    <span>Integrity verified</span>
                    {result.blake3 && (
                      <span className="font-mono text-xs text-neutral-400 truncate max-w-xs">{result.blake3}</span>
                    )}
                  </div>
                ) : (
                  <div className="flex items-center gap-2 text-red-400 text-sm">
                    <Shield className="w-4 h-4" />
                    <span>Integrity check failed</span>
                  </div>
                )}
              </div>
            )}

            {/* Details */}
            {stats && (
              <div className="grid grid-cols-2 gap-4 mb-6">
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700 flex flex-col gap-2">
                  <div className="flex items-center gap-2 text-neutral-300 text-sm">
                    <FileCheck className="w-4 h-4 text-teal-400" />
                    <span>Files:</span>
                  </div>
                  <span className="text-neutral-100 text-lg font-semibold">
                    {statFiles}
                  </span>
                </div>
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700 flex flex-col gap-2">
                  <div className="flex items-center gap-2 text-neutral-300 text-sm">
                    <Archive className="w-4 h-4 text-cyan-400" />
                    <span>Size:</span>
                  </div>
                  <span className="text-neutral-100 text-lg font-semibold">
                    {formatSize(statSize)}
                  </span>
                </div>
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700 flex flex-col gap-2">
                  <div className="flex items-center gap-2 text-neutral-300 text-sm">
                    <Clock className="w-4 h-4 text-violet-400" />
                    <span>Time:</span>
                  </div>
                  <span className="text-neutral-100 text-lg font-semibold">
                    {formatDuration(statDuration)}
                  </span>
                </div>
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700 flex flex-col gap-2">
                  <div className="flex items-center gap-2 text-neutral-300 text-sm">
                    <HardDrive className="w-4 h-4 text-amber-400" />
                    <span>Compression:</span>
                  </div>
                  <span className="text-neutral-100 text-lg font-semibold">
                    {formatRatio(statRatio)}
                  </span>
                </div>
                {statSpeed && (
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700 flex flex-col gap-2">
                  <div className="flex items-center gap-2 text-neutral-300 text-sm">
                    <Upload className="w-4 h-4 text-sky-400" />
                    <span>Speed:</span>
                  </div>
                  <span className="text-neutral-100 text-lg font-semibold">
                    {formatSpeedMb(statSpeed)}
                  </span>
                </div>
                )}
              </div>
            )}

            {/* Error Details */}
            {isError && (
              <div className="mb-6">
                <div className="p-4 rounded-lg bg-neutral-800/30 border border-neutral-700">
                  <h4 className="text-sm font-medium text-neutral-300 mb-2">Error Details</h4>
                  <div className="text-red-400 text-sm font-mono bg-neutral-900/50 p-3 rounded border">
                    {result.error || 'Unknown error'}
                  </div>
                  {result.type === 'password_error' && (
                    <div className="mt-3 p-3 rounded bg-orange-500/10 border border-orange-500/20">
                      <div className="flex items-center gap-2 text-orange-400 text-sm mb-3">
                        <Shield className="w-4 h-4" />
                        <span>Archive is encrypted. Enter password to continue:</span>
                      </div>
                      <div className="space-y-3">
                        <Input
                          type="password"
                          placeholder="Enter archive password..."
                          value={passwordInput}
                          onChange={(e) => setPasswordInput(e.target.value)}
                          className="bg-neutral-800 border-neutral-600 text-white placeholder-neutral-500"
                          disabled={isRetrying}
                        />
                      </div>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Output Path */}
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

            {/* Actions */}
            <div className="flex gap-3">
              {isSuccess && (
                <Button
                  onClick={onClose}
                  className={`flex-1 bg-gradient-to-r ${info.gradient} text-white hover:opacity-90`}
                >
                  Done
                </Button>
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
                      onClick={async () => {
                        if (!passwordInput.trim()) {
                          alert('Please enter a password');
                          return;
                        }
                        
                        setIsRetrying(true);
                        try {
                          if (result.onRetry) {
                            const success = await result.onRetry(passwordInput);
                            if (success) {
                              // DON'T call onClose() - let BlitzArch handle success modal
                              setPasswordInput(''); // Clear password on success
                              // Modal will be closed/reopened by parent component
                            }
                          }
                        } catch (error) {
                          console.error('Retry failed:', error);
                        } finally {
                          setIsRetrying(false);
                        }
                      }}
                      className="flex-1 bg-gradient-to-r from-orange-500 to-amber-500 text-white hover:opacity-90"
                      disabled={isRetrying || !passwordInput.trim()}
                    >
                      {isRetrying ? 'Retrying...' : 'Retry'}
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
