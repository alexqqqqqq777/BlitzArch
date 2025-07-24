import React from 'react';
import { motion } from 'framer-motion';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { Upload, Download, Clock, Activity, CheckCircle } from 'lucide-react';

export default function ProcessingPanel({ progress, speed, type }) {
  const isCompleted = progress >= 100;
  const estimatedTime = speed > 0 ? Math.max(1, Math.ceil((100 - progress) / (speed / 10))) : 0;
  
  const getTypeInfo = () => {
    switch (type) {
      case 'create':
        return {
          icon: Upload,
          title: 'Creating Archive',
          gradient: 'from-emerald-500 to-teal-500',
          bgGradient: 'from-emerald-500/20 to-teal-500/20',
          textColor: 'text-emerald-400'
        };
      case 'extract':
        return {
          icon: Download,
          title: 'Extracting Files',
          gradient: 'from-violet-500 to-purple-500',
          bgGradient: 'from-violet-500/20 to-purple-500/20',
          textColor: 'text-violet-400'
        };
      default:
        return {
          icon: Activity,
          title: 'Processing',
          gradient: 'from-slate-500 to-slate-600',
          bgGradient: 'from-slate-500/20 to-slate-600/20',
          textColor: 'text-slate-400'
        };
    }
  };

  const typeInfo = getTypeInfo();
  const Icon = typeInfo.icon;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -20 }}
      className="bg-slate-900/40 backdrop-blur-md rounded-2xl border border-slate-700/50 p-6"
    >
      <div className="flex items-center gap-3 mb-6">
        <motion.div
          animate={{ rotate: isCompleted ? 0 : 360 }}
          transition={{ duration: 2, repeat: isCompleted ? 0 : Infinity, ease: "linear" }}
          className={`p-2 rounded-lg bg-gradient-to-r ${typeInfo.bgGradient}`}
        >
          {isCompleted ? (
            <CheckCircle className="w-5 h-5 text-emerald-400" />
          ) : (
            <Icon className={`w-5 h-5 ${typeInfo.textColor}`} />
          )}
        </motion.div>
        <h3 className="text-lg font-semibold text-white">
          {isCompleted ? 'Completed!' : typeInfo.title}
        </h3>
      </div>

      {/* Progress Bar */}
      <div className="space-y-3 mb-6">
        <div className="flex items-center justify-between">
          <span className="text-sm text-slate-300">Progress</span>
          <Badge className={`bg-gradient-to-r ${typeInfo.bgGradient} ${typeInfo.textColor} border-0`}>
            {progress.toFixed(1)}%
          </Badge>
        </div>
        
        <div className="relative">
          <Progress value={progress} className="h-2" />
          
          {/* Animated progress wave */}
          {!isCompleted && (
            <motion.div
              className={`absolute top-0 left-0 h-full rounded-full opacity-50 bg-gradient-to-r ${typeInfo.gradient}`}
              style={{ width: '20%' }}
              animate={{ x: ['0%', '400%'] }}
              transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
            />
          )}
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 gap-4">
        <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-800/30">
          <Activity className="w-4 h-4 text-amber-400" />
          <div>
            <div className="text-sm font-medium text-white">
              {speed.toFixed(1)} MB/s
            </div>
            <div className="text-xs text-slate-400">Speed</div>
          </div>
        </div>
        
        <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-800/30">
          <Clock className="w-4 h-4 text-emerald-400" />
          <div>
            <div className="text-sm font-medium text-white">
              {isCompleted ? '0s' : `${estimatedTime}s`}
            </div>
            <div className="text-xs text-slate-400">Remaining</div>
          </div>
        </div>
      </div>

      {/* Status messages */}
      <div className="mt-4 space-y-2">
        <motion.div
          animate={{ opacity: isCompleted ? 1 : [0.5, 1, 0.5] }}
          transition={{ duration: 1.5, repeat: isCompleted ? 0 : Infinity }}
          className="flex items-center gap-2 text-sm text-slate-300"
        >
          <div className={`w-2 h-2 rounded-full ${isCompleted ? 'bg-emerald-400' : `bg-gradient-to-r ${typeInfo.gradient}`}`} />
          {isCompleted 
            ? 'Operation completed successfully'
            : type === 'create' 
            ? 'Compressing and packing files...'
            : 'Unpacking and integrity verification...'
          }
        </motion.div>
      </div>
    </motion.div>
  );
}