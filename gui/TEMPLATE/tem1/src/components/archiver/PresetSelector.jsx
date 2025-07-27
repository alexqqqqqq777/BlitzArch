import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Slider } from '@/components/ui/slider';
import { Badge } from '@/components/ui/badge';
import { motion } from 'framer-motion';
import { Zap, Gauge, Sparkles, Lock, Settings } from 'lucide-react';

const PRESETS = {
  fast: { name: 'Быстро', level: 3, color: '#00FF00', icon: Zap, desc: 'Максимальная скорость' },
  balanced: { name: 'Сбалансированно', level: 9, color: '#FFD700', icon: Gauge, desc: 'Оптимальное соотношение' },
  maximum: { name: 'Максимально', level: 15, color: '#FF6B35', icon: Sparkles, desc: 'Лучшее сжатие' },
  encrypted: { name: 'Зашифрованно', level: 9, color: '#FF0080', icon: Lock, desc: 'Защищённый архив' },
  lzma2: { name: 'LZMA2', level: 9, color: '#8A2BE2', icon: Sparkles, desc: 'Ультра сжатие' }
};

export default function PresetSelector({ preset, onPresetChange, compressionLevel, onCompressionLevelChange }) {
  const currentPreset = PRESETS[preset];

  return (
    <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
      <CardHeader className="pb-4">
        <CardTitle className="text-cyan-300 flex items-center gap-2">
          <Settings className="w-5 h-5" />
          Пресеты компрессии
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        {/* Preset Grid */}
        <div className="grid grid-cols-2 gap-2">
          {Object.entries(PRESETS).map(([key, presetData]) => {
            const Icon = presetData.icon;
            const isActive = preset === key;
            
            return (
              <motion.div
                key={key}
                whileHover={{ scale: 1.02 }}
                whileTap={{ scale: 0.98 }}
              >
                <Button
                  variant={isActive ? "default" : "outline"}
                  className={`w-full h-auto p-3 flex-col gap-2 transition-all ${
                    isActive 
                      ? 'bg-cyan-500 text-black border-cyan-400 shadow-lg shadow-cyan-500/20' 
                      : 'bg-slate-700/50 text-slate-300 border-slate-600 hover:bg-slate-600/50'
                  }`}
                  onClick={() => onPresetChange(key)}
                >
                  <Icon className="w-6 h-6" />
                  <div className="text-center">
                    <div className="font-medium text-sm">{presetData.name}</div>
                    <div className="text-xs opacity-70">{presetData.desc}</div>
                  </div>
                </Button>
              </motion.div>
            );
          })}
        </div>

        {/* Compression Level */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <label className="text-sm font-medium text-slate-300">
              Уровень сжатия
            </label>
            <Badge 
              variant="outline" 
              className="border-slate-600 text-slate-300"
            >
              {compressionLevel}
            </Badge>
          </div>
          
          <Slider
            value={[compressionLevel]}
            onValueChange={(value) => onCompressionLevelChange(value[0])}
            max={22}
            min={1}
            step={1}
            className="w-full"
          />
          
          <div className="flex justify-between text-xs text-slate-500">
            <span>1 (быстро)</span>
            <span>22 (сильно)</span>
          </div>
        </div>

        {/* Current Preset Info */}
        <motion.div
          animate={{ 
            borderColor: currentPreset.color,
            boxShadow: `0 0 10px ${currentPreset.color}20`
          }}
          className="p-3 rounded-lg border bg-slate-700/20"
        >
          <div className="flex items-center gap-2 mb-2">
            <currentPreset.icon className="w-4 h-4" style={{ color: currentPreset.color }} />
            <span className="text-sm font-medium text-slate-200">
              {currentPreset.name}
            </span>
          </div>
          <p className="text-xs text-slate-400">{currentPreset.desc}</p>
        </motion.div>
      </CardContent>
    </Card>
  );
}