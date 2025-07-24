import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { motion } from 'framer-motion';
import { Gauge, Zap } from 'lucide-react';

export default function SpeedGauge({ speed, isActive, title = "Creation Speed" }) {
  const normalizedSpeed = Math.min(speed, 200);
  const angle = (normalizedSpeed / 200) * 180 - 90;
  const speedColor = speed > 150 ? '#00FF00' : speed > 100 ? '#FFD700' : speed > 50 ? '#FF6B35' : '#FF0080';

  return (
    <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
      <CardHeader className="pb-3">
        <CardTitle className="text-cyan-300 flex items-center gap-2 text-lg">
          <Gauge className="w-5 h-5" />
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-4">
        <div className="relative w-48 h-24 mx-auto mb-4">
          {/* Gauge background */}
          <svg viewBox="0 0 200 100" className="w-full h-full">
            {/* Background arc */}
            <path
              d="M 20 80 A 80 80 0 0 1 180 80"
              fill="none"
              stroke="#374151"
              strokeWidth="8"
              strokeLinecap="round"
            />
            
            {/* Speed zones */}
            <path
              d="M 20 80 A 80 80 0 0 1 65 25"
              fill="none"
              stroke="#FF0080"
              strokeWidth="6"
              strokeLinecap="round"
              opacity="0.6"
            />
            <path
              d="M 65 25 A 80 80 0 0 1 100 20"
              fill="none"
              stroke="#FF6B35"
              strokeWidth="6"
              strokeLinecap="round"
              opacity="0.6"
            />
            <path
              d="M 100 20 A 80 80 0 0 1 135 25"
              fill="none"
              stroke="#FFD700"
              strokeWidth="6"
              strokeLinecap="round"
              opacity="0.6"
            />
            <path
              d="M 135 25 A 80 80 0 0 1 180 80"
              fill="none"
              stroke="#00FF00"
              strokeWidth="6"
              strokeLinecap="round"
              opacity="0.6"
            />

            {/* Needle */}
            <motion.line
              x1="100"
              y1="80"
              x2="100"
              y2="30"
              stroke={speedColor}
              strokeWidth="3"
              strokeLinecap="round"
              filter="drop-shadow(0 0 4px currentColor)"
              animate={{
                rotate: angle,
                stroke: speedColor
              }}
              transition={{
                type: "spring",
                stiffness: 100,
                damping: 10
              }}
              style={{
                transformOrigin: "100px 80px"
              }}
            />
            
            {/* Center dot */}
            <circle
              cx="100"
              cy="80"
              r="4"
              fill={speedColor}
              className="drop-shadow-lg"
            />
          </svg>
        </div>

        {/* Speed display */}
        <div className="text-center">
          <motion.div
            animate={{
              scale: isActive ? [1, 1.05, 1] : 1,
              color: speedColor
            }}
            transition={{
              duration: 0.8,
              repeat: isActive ? Infinity : 0
            }}
            className="text-3xl font-bold font-mono mb-1"
          >
            {speed.toFixed(1)}
          </motion.div>
          <div className="text-sm text-slate-400">MB/s</div>
        </div>

        {/* Speed indicators */}
        <div className="flex justify-between text-xs text-slate-500 mt-2">
          <span>0</span>
          <span>50</span>
          <span>100</span>
          <span>150</span>
          <span>200</span>
        </div>

        {/* Status indicator */}
        {isActive && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="flex items-center justify-center gap-2 mt-3 text-cyan-400"
          >
            <motion.div
              animate={{ rotate: 360 }}
              transition={{ duration: 1, repeat: Infinity, ease: "linear" }}
            >
              <Zap className="w-4 h-4" />
            </motion.div>
            <span className="text-sm font-medium">Active</span>
          </motion.div>
        )}
      </CardContent>
    </Card>
  );
}