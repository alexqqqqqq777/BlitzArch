import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { motion } from 'framer-motion';
import { Zap, Download, Upload, Clock, HardDrive } from 'lucide-react';

export default function TaskProgress({ progress, speed, isCreating, isExtracting }) {
  const taskType = isCreating ? 'create' : 'extract';
  const taskIcon = isCreating ? Upload : Download;
  const taskColor = isCreating ? '#00FFFF' : '#FFD700';
  const taskName = isCreating ? 'Создание архива' : 'Извлечение файлов';

  const getProgressColor = () => {
    if (progress < 25) return 'bg-red-500';
    if (progress < 50) return 'bg-yellow-500';
    if (progress < 75) return 'bg-blue-500';
    return 'bg-green-500';
  };

  const estimatedTime = speed > 0 ? Math.max(1, Math.ceil((100 - progress) / (speed / 10))) : 0;

  return (
    <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
      <CardHeader className="pb-4">
        <CardTitle className="flex items-center gap-2" style={{ color: taskColor }}>
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
          >
            <taskIcon className="w-5 h-5" />
          </motion.div>
          {taskName}
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        {/* Main Progress Bar */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="text-sm text-slate-300">Прогресс</span>
            <Badge 
              variant="outline" 
              className="border-slate-600 text-slate-300"
            >
              {progress.toFixed(1)}%
            </Badge>
          </div>
          
          <motion.div
            initial={{ width: 0 }}
            animate={{ width: "100%" }}
            className="relative"
          >
            <Progress 
              value={progress} 
              className="h-3"
            />
            <motion.div
              className="absolute top-0 left-0 h-full rounded-full opacity-30"
              style={{ 
                background: `linear-gradient(90deg, ${taskColor}00, ${taskColor}ff, ${taskColor}00)`,
                width: '20%'
              }}
              animate={{
                x: [`-20%`, `100%`]
              }}
              transition={{
                duration: 2,
                repeat: Infinity,
                ease: "linear"
              }}
            />
          </motion.div>
        </div>

        {/* Stats Grid */}
        <div className="grid grid-cols-2 gap-4">
          <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-700/30">
            <Zap className="w-4 h-4 text-yellow-400" />
            <div>
              <div className="text-sm font-medium text-slate-200">
                {speed.toFixed(1)} MB/s
              </div>
              <div className="text-xs text-slate-500">Скорость</div>
            </div>
          </div>
          
          <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-700/30">
            <Clock className="w-4 h-4 text-blue-400" />
            <div>
              <div className="text-sm font-medium text-slate-200">
                {estimatedTime}s
              </div>
              <div className="text-xs text-slate-500">Осталось</div>
            </div>
          </div>
        </div>

        {/* Status Messages */}
        <div className="space-y-2">
          <motion.div
            animate={{ opacity: [0.5, 1, 0.5] }}
            transition={{ duration: 1.5, repeat: Infinity }}
            className="flex items-center gap-2 text-sm text-slate-300"
          >
            <div className="w-2 h-2 rounded-full bg-cyan-400"></div>
            {isCreating ? 'Сжатие файлов...' : 'Распаковка архива...'}
          </motion.div>
          
          {progress > 50 && (
            <motion.div
              initial={{ opacity: 0, x: -20 }}
              animate={{ opacity: 1, x: 0 }}
              className="flex items-center gap-2 text-sm text-green-400"
            >
              <div className="w-2 h-2 rounded-full bg-green-400"></div>
              Проверка целостности данных
            </motion.div>
          )}
        </div>

        {/* Animated Progress Ring */}
        <div className="flex justify-center">
          <motion.div
            className="relative w-16 h-16"
            animate={{ rotate: 360 }}
            transition={{ duration: 3, repeat: Infinity, ease: "linear" }}
          >
            <svg viewBox="0 0 64 64" className="w-full h-full">
              <circle
                cx="32"
                cy="32"
                r="28"
                fill="none"
                stroke="#374151"
                strokeWidth="4"
              />
              <motion.circle
                cx="32"
                cy="32"
                r="28"
                fill="none"
                stroke={taskColor}
                strokeWidth="4"
                strokeDasharray={`${2 * Math.PI * 28}`}
                strokeDashoffset={`${2 * Math.PI * 28 * (1 - progress / 100)}`}
                strokeLinecap="round"
                filter="drop-shadow(0 0 4px currentColor)"
                style={{ transformOrigin: "32px 32px", transform: "rotate(-90deg)" }}
              />
            </svg>
          </motion.div>
        </div>
      </CardContent>
    </Card>
  );
}