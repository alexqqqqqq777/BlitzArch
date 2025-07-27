import React from 'react';
import { motion } from 'framer-motion';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { 
  Upload, 
  Download, 
  Clock, 
  Activity, 
  CheckCircle,
  Zap,
  HardDrive
} from 'lucide-react';

export default function ProcessingInterface({ progress, speed, type }) {
  const isCompleted = progress >= 100;
  const estimatedTime = speed > 0 ? Math.max(1, Math.ceil((100 - progress) / (speed / 10))) : 0;
  
  const getTypeInfo = () => {
    switch (type) {
      case 'create':
        return {
          icon: Upload,
          title: 'Creating Archive',
          subtitle: 'Compressing and packaging files...',
          gradient: 'from-teal-500 to-cyan-500',
          bgGradient: 'from-teal-500/20 to-cyan-500/20',
          textColor: 'text-teal-400'
        };
      case 'extract':
        return {
          icon: Download,
          title: 'Extracting Archive',
          subtitle: 'Decompressing and extracting files...',
          gradient: 'from-violet-500 to-purple-500',
          bgGradient: 'from-violet-500/20 to-purple-500/20',
          textColor: 'text-violet-400'
        };
      default:
        return {
          icon: Activity,
          title: 'Processing',
          subtitle: 'Operation in progress...',
          gradient: 'from-neutral-500 to-neutral-600',
          bgGradient: 'from-neutral-500/20 to-neutral-600/20',
          textColor: 'text-neutral-400'
        };
    }
  };

  const typeInfo = getTypeInfo();
  const Icon = typeInfo.icon;

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      className="absolute inset-0 z-50 flex items-center justify-center bg-neutral-900/80 backdrop-blur-sm rounded-2xl"
    >
      <div className="w-full max-w-md p-8">
        
        {/* Header */}
        <div className="text-center mb-8">
          <motion.div
            animate={{ 
              rotate: isCompleted ? 0 : 360,
              scale: isCompleted ? 1.1 : [1, 1.05, 1]
            }}
            transition={{ 
              rotate: { duration: 2, repeat: isCompleted ? 0 : Infinity, ease: "linear" },
              scale: { duration: 1, repeat: isCompleted ? 0 : Infinity }
            }}
            className={`w-16 h-16 mx-auto mb-4 rounded-xl flex items-center justify-center bg-gradient-to-r ${typeInfo.bgGradient} border border-neutral-600`}
          >
            {isCompleted ? (
              <CheckCircle className="w-8 h-8 text-emerald-400" />
            ) : (
              <Icon className={`w-8 h-8 ${typeInfo.textColor}`} />
            )}
          </motion.div>
          
          <h3 className="text-xl font-bold text-white mb-2">
            {isCompleted ? 'Operation Complete!' : typeInfo.title}
          </h3>
          <p className="text-neutral-400 text-sm">
            {isCompleted ? 'Files processed successfully' : typeInfo.subtitle}
          </p>
        </div>

        {/* Progress Circle */}
        <div className="relative w-32 h-32 mx-auto mb-8">
          <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
            {/* Background circle */}
            <circle
              cx="50"
              cy="50"
              r="45"
              fill="none"
              stroke="rgba(115, 115, 115, 0.2)"
              strokeWidth="6"
            />
            
            {/* Progress circle */}
            <motion.circle
              cx="50"
              cy="50"
              r="45"
              fill="none"
              stroke="url(#progressGradient)"
              strokeWidth="6"
              strokeLinecap="round"
              strokeDasharray={`${2 * Math.PI * 45}`}
              initial={{ strokeDashoffset: `${2 * Math.PI * 45}` }}
              animate={{ 
                strokeDashoffset: `${2 * Math.PI * 45 * (1 - progress / 100)}`
              }}
              transition={{ duration: 0.5, ease: "easeOut" }}
            />
            
            <defs>
              <linearGradient id="progressGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stopColor="#14B8A6" />
                <stop offset="50%" stopColor="#06B6D4" />
                <stop offset="100%" stopColor="#8B5CF6" />
              </linearGradient>
            </defs>
          </svg>
          
          {/* Center display */}
          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <motion.div
              animate={{ scale: [1, 1.1, 1] }}
              transition={{ duration: 1, repeat: isCompleted ? 0 : Infinity }}
              className={`text-2xl font-bold ${typeInfo.textColor}`}
            >
              {progress.toFixed(0)}%
            </motion.div>
          </div>
        </div>

        {/* Stats Grid */}
        <div className="grid grid-cols-2 gap-4 mb-6">
          <div className="flex items-center gap-2 p-3 rounded-lg bg-neutral-800/50">
            <Zap className="w-4 h-4 text-amber-400" />
            <div>
              <div className="text-sm font-medium text-white">
                {speed.toFixed(1)} MB/s
              </div>
              <div className="text-xs text-neutral-400">Speed</div>
            </div>
          </div>
          
          <div className="flex items-center gap-2 p-3 rounded-lg bg-neutral-800/50">
            <Clock className="w-4 h-4 text-emerald-400" />
            <div>
              <div className="text-sm font-medium text-white">
                {isCompleted ? '0s' : `${estimatedTime}s`}
              </div>
              <div className="text-xs text-neutral-400">Remaining</div>
            </div>
          </div>
        </div>

        {/* Progress Bar */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm text-neutral-300">Progress</span>
            <Badge className={`bg-gradient-to-r ${typeInfo.bgGradient} ${typeInfo.textColor} border-0`}>
              {progress.toFixed(1)}%
            </Badge>
          </div>
          
          <div className="relative">
            <Progress value={progress} className="h-3" />
            
            {/* Animated progress wave */}
            {!isCompleted && (
              <motion.div
                className={`absolute top-0 left-0 h-full rounded-full opacity-50 bg-gradient-to-r ${typeInfo.gradient}`}
                style={{ width: '25%' }}
                animate={{ x: ['0%', '300%'] }}
                transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
              />
            )}
          </div>
        </div>

        {/* Status Message */}
        <motion.div
          animate={{ opacity: isCompleted ? 1 : [0.7, 1, 0.7] }}
          transition={{ duration: 1.5, repeat: isCompleted ? 0 : Infinity }}
          className="mt-6 text-center"
        >
          <div className="flex items-center justify-center gap-2 text-sm text-neutral-300">
            <div className={`w-2 h-2 rounded-full ${isCompleted ? 'bg-emerald-400' : `bg-gradient-to-r ${typeInfo.gradient}`}`} />
            {isCompleted 
              ? 'Operation completed successfully'
              : type === 'create' 
              ? 'Compressing files and creating archive...'
              : 'Extracting files and verifying integrity...'
            }
          </div>
        </motion.div>
      </div>
    </motion.div>
  );
}