import React from 'react';
import { motion } from 'framer-motion';
import { Gauge, Zap, TrendingUp } from 'lucide-react';

export default function SpeedMeter({ speed, isActive, type }) {
  const maxSpeed = 200;
  const normalizedSpeed = Math.min(speed, maxSpeed);
  const percentage = (normalizedSpeed / maxSpeed) * 100;
  
  const getSpeedColor = () => {
    if (speed < 30) return { text: 'text-red-400', bg: 'from-red-500 to-red-600' };
    if (speed < 60) return { text: 'text-amber-400', bg: 'from-amber-500 to-orange-500' };
    if (speed < 100) return { text: 'text-emerald-400', bg: 'from-emerald-500 to-teal-500' };
    return { text: 'text-violet-400', bg: 'from-violet-500 to-purple-500' };
  };

  const getSpeedLevel = () => {
    if (speed < 30) return 'Slow';
    if (speed < 60) return 'Normal';
    if (speed < 100) return 'Fast';
    return 'Very Fast';
  };

  const speedColor = getSpeedColor();

  return (
    <div className="bg-slate-900/40 backdrop-blur-md rounded-2xl border border-slate-700/50 p-6">
      <div className="flex items-center gap-3 mb-6">
        <Gauge className="w-5 h-5 text-slate-400" />
        <h3 className="text-lg font-semibold text-white">
          {type === 'create' ? 'Creation Speed' : type === 'extract' ? 'Extraction Speed' : 'Processing Speed'}
        </h3>
      </div>

      {/* Circular Progress */}
      <div className="relative w-32 h-32 mx-auto mb-6">
        <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
          {/* Background circle */}
          <circle
            cx="50"
            cy="50"
            r="45"
            fill="none"
            stroke="rgba(71, 85, 105, 0.3)"
            strokeWidth="8"
          />
          
          {/* Progress circle */}
          <motion.circle
            cx="50"
            cy="50"
            r="45"
            fill="none"
            stroke="url(#speedGradient)"
            strokeWidth="8"
            strokeLinecap="round"
            strokeDasharray={`${2 * Math.PI * 45}`}
            initial={{ strokeDashoffset: `${2 * Math.PI * 45}` }}
            animate={{ 
              strokeDashoffset: `${2 * Math.PI * 45 * (1 - percentage / 100)}`
            }}
            transition={{ duration: 0.5, ease: "easeOut" }}
          />
          
          <defs>
            <linearGradient id="speedGradient" x1="0%" y1="0%" x2="100%" y2="100%">
              <stop offset="0%" stopColor="#F59E0B" />
              <stop offset="50%" stopColor="#EF4444" />
              <stop offset="100%" stopColor="#8B5CF6" />
            </linearGradient>
          </defs>
        </svg>
        
        {/* Speed display */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <motion.div
            animate={{ scale: isActive ? [1, 1.1, 1] : 1 }}
            transition={{ duration: 1, repeat: isActive ? Infinity : 0 }}
            className={`text-2xl font-bold ${speedColor.text}`}
          >
            {speed.toFixed(0)}
          </motion.div>
          <div className="text-sm text-slate-400">MB/s</div>
        </div>
      </div>

      {/* Speed Status */}
      <div className="text-center space-y-2">
        <div className={`inline-flex items-center gap-2 px-3 py-1 rounded-full text-sm font-medium ${
          isActive 
            ? 'bg-gradient-to-r from-emerald-500/20 to-teal-500/20 text-emerald-400 border border-emerald-500/30' 
            : 'bg-slate-800/50 text-slate-400 border border-slate-600/50'
        }`}>
          {isActive ? (
            <>
              <motion.div
                animate={{ rotate: 360 }}
                transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
              >
                <Zap className="w-4 h-4" />
              </motion.div>
              Active
            </>
          ) : (
            <>
              <TrendingUp className="w-4 h-4" />
              Ready
            </>
          )}
        </div>
        
        <p className={`text-sm font-medium ${speedColor.text}`}>
          {getSpeedLevel()}
        </p>
      </div>
    </div>
  );
}