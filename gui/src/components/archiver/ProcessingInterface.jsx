import React from 'react';
import { motion } from 'framer-motion';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { 
  Upload, 
  Download, 
  CheckCircle, 
  Activity,
  Zap,
  Database,
  Lock
} from 'lucide-react';

export default function ProcessingInterface({ progress, speed, type, finalMessage }) {
  const isCompleted = progress >= 100;
  
  const getProcessInfo = () => {
    switch (type) {
      case 'create':
        return {
          title: 'Creating Archive',
          subtitle: 'Compressing and packaging files',
          icon: Upload,
          color: 'from-teal-500 to-cyan-500',
          bgColor: 'from-teal-500/20 to-cyan-500/20'
        };
      case 'extract':
        return {
          title: 'Extracting Archive',
          subtitle: 'Decompressing and validating files',
          icon: Download,
          color: 'from-emerald-500 to-teal-500',
          bgColor: 'from-emerald-500/20 to-teal-500/20'
        };
      default:
        return {
          title: 'Processing',
          subtitle: 'Working on your files',
          icon: Activity,
          color: 'from-neutral-400 to-neutral-500',
          bgColor: 'from-neutral-500/20 to-neutral-600/20'
        };
    }
  };

  const processInfo = getProcessInfo();
  const Icon = processInfo.icon;

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      className="absolute inset-0 z-20 bg-neutral-900/95 backdrop-blur-sm rounded-2xl flex items-center justify-center"
    >
      <div className="text-center max-w-md p-8">
        
        {/* Status Icon */}
        <motion.div
          animate={{ 
            rotate: isCompleted ? 0 : 360,
            scale: isCompleted ? [1, 1.2, 1] : 1
          }}
          transition={{ 
            rotate: { duration: 2, repeat: isCompleted ? 0 : Infinity, ease: "linear" },
            scale: { duration: 0.5 }
          }}
          className={`w-20 h-20 mx-auto mb-6 rounded-2xl flex items-center justify-center bg-gradient-to-br ${
            isCompleted ? 'from-emerald-500 to-teal-500' : processInfo.color
          } shadow-2xl`}
        >
          {isCompleted ? (
            <CheckCircle className="w-10 h-10 text-white" />
          ) : (
            <Icon className="w-10 h-10 text-white" />
          )}
        </motion.div>

        {/* Title */}
        <h3 className="text-2xl font-bold text-white mb-2">
          {isCompleted ? 'Operation Complete!' : processInfo.title}
        </h3>
        
        <p className="text-neutral-400 mb-8">
          {isCompleted ? (finalMessage || 'Your files have been processed successfully') : processInfo.subtitle}
        </p>

        {/* Progress Bar */}
        <div className="mb-6">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-neutral-300">Progress</span>
            <Badge className={`bg-gradient-to-r ${processInfo.bgColor} text-white border-0`}>
              {progress.toFixed(1)}%
            </Badge>
          </div>
          
          <div className="relative">
            <Progress value={progress} className="h-3 mb-2" />
            
            {/* Animated progress glow */}
            {!isCompleted && (
              <motion.div
                className={`absolute top-0 left-0 h-3 rounded-full bg-gradient-to-r ${processInfo.color} opacity-30`}
                style={{ width: '30%' }}
                animate={{ x: ['0%', '250%'] }}
                transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
              />
            )}
          </div>
        </div>

        {/* Processing Stats */}
        <div className="grid grid-cols-3 gap-4 mb-6">
          <div className="text-center p-3 bg-neutral-800/50 rounded-lg">
            <Zap className="w-4 h-4 text-teal-400 mx-auto mb-1" />
            <div className="text-sm font-semibold text-white">{speed.toFixed(1)}</div>
            <div className="text-xs text-neutral-400">MB/s</div>
          </div>
          
          <div className="text-center p-3 bg-neutral-800/50 rounded-lg">
            <Database className="w-4 h-4 text-cyan-400 mx-auto mb-1" />
            <div className="text-sm font-semibold text-white">
              {type === 'create' ? '3.2:1' : '1:3.2'}
            </div>
            <div className="text-xs text-neutral-400">Ratio</div>
          </div>
          
          <div className="text-center p-3 bg-neutral-800/50 rounded-lg">
            <Lock className="w-4 h-4 text-emerald-400 mx-auto mb-1" />
            <div className="text-sm font-semibold text-white">{Math.round(progress)}</div>
            <div className="text-xs text-neutral-400">Files</div>
          </div>
        </div>

        {/* Status Messages */}
        <div className="text-left space-y-2">
          <motion.div
            animate={{ opacity: [0.5, 1, 0.5] }}
            transition={{ duration: 2, repeat: isCompleted ? 0 : Infinity }}
            className="flex items-center gap-2 text-sm text-neutral-300"
          >
            <div className={`w-2 h-2 rounded-full ${
              isCompleted ? 'bg-emerald-400' : `bg-gradient-to-r ${processInfo.color}`
            }`} />
            {isCompleted 
              ? (finalMessage || 'All operations completed successfully')
              : type === 'create' 
              ? 'Analyzing and compressing file data...'
              : 'Extracting and verifying file integrity...'
            }
          </motion.div>
          
          {progress > 30 && !isCompleted && (
            <motion.div
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center gap-2 text-sm text-neutral-400"
            >
              <div className="w-2 h-2 rounded-full bg-teal-400" />
              {type === 'create' ? 'Applying compression algorithms...' : 'Validating BLAKE3 integrity...'}
            </motion.div>
          )}
          
          {progress > 70 && !isCompleted && (
            <motion.div
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center gap-2 text-sm text-neutral-400"
            >
              <div className="w-2 h-2 rounded-full bg-cyan-400" />
              {type === 'create' ? 'Finalizing archive structure...' : 'Completing file extraction...'}
            </motion.div>
          )}
        </div>
      </div>
    </motion.div>
  );
}