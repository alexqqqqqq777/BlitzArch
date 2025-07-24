import React from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Settings, Lock, Zap, Shield, Gauge, Sparkles } from 'lucide-react';

const PRESETS = {
  fast: { 
    name: 'Fast', 
    level: 3, 
    icon: Zap, 
    gradient: 'from-emerald-500 to-teal-500',
    bgGradient: 'from-emerald-500/20 to-teal-500/20',
    border: 'border-emerald-500/30'
  },
  balanced: { 
    name: 'Balanced', 
    level: 9, 
    icon: Gauge, 
    gradient: 'from-amber-500 to-orange-500',
    bgGradient: 'from-amber-500/20 to-orange-500/20',
    border: 'border-amber-500/30'
  },
  maximum: { 
    name: 'Maximum', 
    level: 15, 
    icon: Sparkles, 
    gradient: 'from-violet-500 to-purple-500',
    bgGradient: 'from-violet-500/20 to-purple-500/20',
    border: 'border-violet-500/30'
  },
  encrypted: { 
    name: 'Encrypted', 
    level: 9, 
    icon: Lock, 
    gradient: 'from-red-500 to-rose-500',
    bgGradient: 'from-red-500/20 to-rose-500/20',
    border: 'border-red-500/30'
  }
};

export default function ControlPanel({ settings, onSettingsChange, disabled }) {
  const updateSetting = (key, value) => {
    onSettingsChange(prev => ({ ...prev, [key]: value }));
  };

  const handlePresetChange = (preset) => {
    updateSetting('preset', preset);
    updateSetting('compressionLevel', PRESETS[preset].level);
    if (preset === 'encrypted') {
      updateSetting('useEncryption', true);
    }
  };

  const currentPreset = PRESETS[settings.preset];

  return (
    <div className="bg-slate-900/40 backdrop-blur-md rounded-2xl border border-slate-700/50 p-6">
      <div className="flex items-center gap-3 mb-6">
        <Settings className="w-5 h-5 text-slate-400" />
        <h3 className="text-lg font-semibold text-white">Settings</h3>
      </div>

      <div className="space-y-6">
        
        {/* Presets */}
        <div>
          <label className="text-sm font-medium text-slate-300 mb-3 block">
            Compression Presets
          </label>
          <div className="grid grid-cols-2 gap-2">
            {Object.entries(PRESETS).map(([key, preset]) => {
              const Icon = preset.icon;
              const isActive = settings.preset === key;
              
              return (
                <Button
                  key={key}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  disabled={disabled}
                  onClick={() => handlePresetChange(key)}
                  className={`h-auto p-3 flex-col gap-2 transition-all ${
                    isActive 
                      ? `bg-gradient-to-r ${preset.gradient} text-white shadow-lg` 
                      : 'bg-slate-800/30 text-slate-300 border-slate-600/50 hover:bg-slate-700/40'
                  }`}
                >
                  <Icon className="w-4 h-4" />
                  <span className="text-xs font-medium">{preset.name}</span>
                </Button>
              );
            })}
          </div>
        </div>

        {/* Compression Level */}
        <div>
          <div className="flex items-center justify-between mb-3">
            <label className="text-sm font-medium text-slate-300">
              Compression Level
            </label>
            <Badge className="bg-slate-800/50 text-amber-400 border border-amber-500/30">
              {settings.compressionLevel}
            </Badge>
          </div>
          
          <Slider
            value={[settings.compressionLevel]}
            onValueChange={(value) => updateSetting('compressionLevel', value[0])}
            max={22}
            min={1}
            step={1}
            disabled={disabled}
            className="mb-2"
          />
          
          <div className="flex justify-between text-xs text-slate-500">
            <span>Fast</span>
            <span>Maximum</span>
          </div>
        </div>

        {/* Encryption */}
        <div>
          <div className="flex items-center justify-between mb-3">
            <label className="text-sm font-medium text-slate-300 flex items-center gap-2">
              <Shield className="w-4 h-4" />
              Encryption
            </label>
            <Switch
              checked={settings.useEncryption}
              onCheckedChange={(checked) => updateSetting('useEncryption', checked)}
              disabled={disabled}
            />
          </div>
          
          {settings.useEncryption && (
            <div className="relative">
              <Lock className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-400 w-4 h-4" />
              <Input
                type="password"
                placeholder="Enter password"
                value={settings.password}
                onChange={(e) => updateSetting('password', e.target.value)}
                disabled={disabled}
                className="pl-10 bg-slate-800/30 border-slate-600/50 text-white placeholder-slate-400"
              />
            </div>
          )}
        </div>

        {/* Info */}
        <div className={`p-4 rounded-xl bg-gradient-to-r ${currentPreset.bgGradient} border ${currentPreset.border}`}>
          <div className="flex items-center gap-2 mb-2">
            <div className={`w-3 h-3 rounded-full bg-gradient-to-r ${currentPreset.gradient}`} />
            <span className="text-sm font-medium text-white">
              {currentPreset.name}
            </span>
          </div>
          <p className="text-xs text-slate-300">
            Level {settings.compressionLevel} • {settings.useEncryption ? 'Encrypted' : 'No Encryption'}
          </p>
        </div>
      </div>
    </div>
  );
}