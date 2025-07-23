import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { motion } from 'framer-motion';
import tauriEngine from '../../lib/tauri-engine';
import { 
  Gauge, 
  Activity, 
  Clock, 
  TrendingUp,
  Cpu,
  MemoryStick,
  HardDrive
} from 'lucide-react';

export default function MetricsPanel({ speed, progress, isActive, type }) {
  const [systemMetrics, setSystemMetrics] = useState({
    cpu_usage: 0,
    memory_percentage: 0,
    memory_used: 0,
    memory_total: 0,
    disk_usage: 0
  });
  
  // Periodically fetch system metrics
  useEffect(() => {
    const fetchSystemMetrics = async () => {
      try {
        const metrics = await tauriEngine.getSystemMetrics();
        setSystemMetrics(metrics);
      } catch (error) {
        console.error('Failed to fetch system metrics:', error);
      }
    };
    
    // Initial fetch
    fetchSystemMetrics();
    
    // Set up periodic fetching every 2 seconds
    const interval = setInterval(fetchSystemMetrics, 2000);
    
    return () => clearInterval(interval);
  }, []);
  
  const maxSpeed = 250;
  const normalizedSpeed = Math.min(speed, maxSpeed);
  const percentage = (normalizedSpeed / maxSpeed) * 100;
  
  const getSpeedStatus = () => {
    if (speed < 25) return { label: 'Slow', color: 'text-red-400', bg: 'bg-red-500/20' };
    if (speed < 60) return { label: 'Normal', color: 'text-yellow-400', bg: 'bg-yellow-500/20' };
    if (speed < 120) return { label: 'Fast', color: 'text-teal-400', bg: 'bg-teal-500/20' };
    return { label: 'Blazing', color: 'text-cyan-400', bg: 'bg-cyan-500/20' };
  };

  const status = getSpeedStatus();
  const estimatedTime = speed > 0 ? Math.max(1, Math.ceil((100 - progress) / (speed / 8))) : 0;

  return (
    <div className="space-y-6">
      
      {/* Main Speed Gauge */}
      <Card className="bg-neutral-800/40 border-neutral-700">
        <CardHeader className="pb-4">
          <CardTitle className="text-white flex items-center gap-3">
            <Gauge className="w-5 h-5 text-teal-400" />
            Performance Monitor
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          
          {/* Circular Gauge */}
          <div className="relative w-40 h-40 mx-auto">
            <svg className="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
              {/* Background track */}
              <circle
                cx="50"
                cy="50"
                r="42"
                fill="none"
                stroke="rgba(115, 115, 115, 0.2)"
                strokeWidth="6"
              />
              
              {/* Progress track */}
              <motion.circle
                cx="50"
                cy="50"
                r="42"
                fill="none"
                stroke="url(#speedGradient)"
                strokeWidth="6"
                strokeLinecap="round"
                strokeDasharray={`${2 * Math.PI * 42}`}
                initial={{ strokeDashoffset: `${2 * Math.PI * 42}` }}
                animate={{ 
                  strokeDashoffset: `${2 * Math.PI * 42 * (1 - percentage / 100)}`
                }}
                transition={{ duration: 0.8, ease: "easeOut" }}
              />
              
              <defs>
                <linearGradient id="speedGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                  <stop offset="0%" stopColor="#14B8A6" />
                  <stop offset="50%" stopColor="#06B6D4" />
                  <stop offset="100%" stopColor="#3B82F6" />
                </linearGradient>
              </defs>
            </svg>
            
            {/* Center display */}
            <div className="absolute inset-0 flex flex-col items-center justify-center">
              <motion.div
                animate={{ scale: isActive ? [1, 1.05, 1] : 1 }}
                transition={{ duration: 1.5, repeat: isActive ? Infinity : 0 }}
                className={`text-3xl font-bold ${status.color}`}
              >
                {speed.toFixed(0)}
              </motion.div>
              <div className="text-sm text-neutral-400 font-medium">MB/s</div>
              <Badge className={`mt-2 ${status.bg} ${status.color} border-0 text-xs`}>
                {status.label}
              </Badge>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* System Metrics */}
      <Card className="bg-neutral-800/40 border-neutral-700">
        <CardHeader className="pb-4">
          <CardTitle className="text-white flex items-center gap-3">
            <Activity className="w-5 h-5 text-cyan-400" />
            System Metrics
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          
          {/* CPU Usage */}
          <div className="flex items-center justify-between p-3 bg-neutral-700/30 rounded-lg">
            <div className="flex items-center gap-3">
              <Cpu className="w-4 h-4 text-teal-400" />
              <span className="text-neutral-300 text-sm font-medium">CPU Usage</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-16 h-2 bg-neutral-600 rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-gradient-to-r from-teal-500 to-cyan-500"
                  initial={{ width: '0%' }}
                  animate={{ width: `${Math.min(systemMetrics.cpu_usage, 100)}%` }}
                />
              </div>
              <span className="text-neutral-400 text-xs w-8">
                {Math.round(systemMetrics.cpu_usage)}%
              </span>
            </div>
          </div>

          {/* Memory Usage */}
          <div className="flex items-center justify-between p-3 bg-neutral-700/30 rounded-lg">
            <div className="flex items-center gap-3">
              <MemoryStick className="w-4 h-4 text-emerald-400" />
              <span className="text-neutral-300 text-sm font-medium">Memory</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-16 h-2 bg-neutral-600 rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-gradient-to-r from-emerald-500 to-teal-500"
                  initial={{ width: '0%' }}
                  animate={{ width: `${Math.min(systemMetrics.memory_percentage, 100)}%` }}
                />
              </div>
              <span className="text-neutral-400 text-xs w-12">
                {(systemMetrics.memory_used / (1024 * 1024 * 1024)).toFixed(1)}GB
              </span>
            </div>
          </div>

          {/* Disk I/O */}
          <div className="flex items-center justify-between p-3 bg-neutral-700/30 rounded-lg">
            <div className="flex items-center gap-3">
              <HardDrive className="w-4 h-4 text-cyan-400" />
              <span className="text-neutral-300 text-sm font-medium">Disk I/O</span>
            </div>
            <div className="flex items-center gap-2">
              <div className="w-16 h-2 bg-neutral-600 rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-gradient-to-r from-cyan-500 to-blue-500"
                  initial={{ width: '0%' }}
                  animate={{ width: `${Math.min(systemMetrics.disk_usage, 100)}%` }}
                />
              </div>
              <span className="text-neutral-400 text-xs w-12">
                {Math.round(systemMetrics.disk_usage)}%
              </span>
            </div>
          </div>

          {/* ETA */}
          {isActive && (
            <motion.div 
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              className="flex items-center justify-between p-3 bg-gradient-to-r from-teal-500/10 to-cyan-500/10 rounded-lg border border-teal-500/20"
            >
              <div className="flex items-center gap-3">
                <Clock className="w-4 h-4 text-teal-400" />
                <span className="text-teal-300 text-sm font-medium">Estimated Time</span>
              </div>
              <span className="text-teal-400 text-sm font-bold">
                {estimatedTime}s remaining
              </span>
            </motion.div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}