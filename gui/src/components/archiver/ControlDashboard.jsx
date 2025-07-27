import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { 
  Settings, 
  Zap, 
  Shield, 
  Gauge, 
  Sparkles, 
  Lock,
  Sliders,
  Cpu
} from 'lucide-react';

const COMPRESSION_PROFILES = {
  balanced: { 
    name: 'Fast', 
    level: -3,                   // Fast profile: zstd fast=3 (-3)
    icon: Gauge, 
    color: 'from-teal-400 to-cyan-500',
    desc: 'Fast default compression',

    threads: 32,                 // Fixed 32 threads
    codecThreads: 16,            // Fixed 16 codec threads
    useEncryption: false,
    memoryBudget: 0
  },
  high: { 
    name: 'Balanced', 
    level: 3,                   // Balanced level
    icon: Sparkles, 
    color: 'from-purple-400 to-pink-500',
    desc: 'Balanced compression',

    threads: 0,                 // Auto threads
    codecThreads: 0,            // Auto codec threads
    useEncryption: false,
    memoryBudget: 50
  },
  maximum: { 
    name: 'Maximum', 
    level: 7,                   // Maximum compression
    icon: Zap, 
    color: 'from-yellow-400 to-orange-500',
    desc: 'Best compression, slower speed',

    threads: 0,                 // Auto threads
    codecThreads: 0,            // Auto codec threads
    useEncryption: false,
    memoryBudget: 70
  },
  secure: { 
    name: 'Secure', 
    level: 3,                   // Password mode
    icon: Shield, 
    color: 'from-red-400 to-rose-500',
    desc: 'AES-256 encrypted archive',

    threads: 0,                 // Auto threads
    codecThreads: 0,            // Auto codec threads
    useEncryption: true,         // Force encryption for secure profile
    memoryBudget: 70
  }
};

export default function ControlDashboard({ settings, onSettingsChange, disabled }) {
  const updateSetting = (key, value) => {
    onSettingsChange(prev => ({ ...prev, [key]: value }));
  };

  const handleProfileChange = (profile) => {
    const profileConfig = COMPRESSION_PROFILES[profile];
    
    // Apply all profile parameters at once
    updateSetting('preset', profile);
    updateSetting('compressionLevel', profileConfig.level);

    updateSetting('threads', profileConfig.threads);
    updateSetting('memoryBudget', profileConfig.memoryBudget);
    updateSetting('codecThreads', profileConfig.codecThreads);
    updateSetting('useEncryption', profileConfig.useEncryption);
    
    // Clear password if encryption is disabled
    if (!profileConfig.useEncryption) {
      updateSetting('password', '');
    }
  };

  const currentProfile = COMPRESSION_PROFILES[settings.preset] || COMPRESSION_PROFILES.balanced;
  // Determine if manual changes were made on top of profile
  const isModified = (
    settings.compressionLevel !== currentProfile.level ||

    settings.memoryBudget !== currentProfile.memoryBudget ||
    settings.threads !== currentProfile.threads ||
    settings.codecThreads !== currentProfile.codecThreads ||
    settings.useEncryption !== currentProfile.useEncryption
  );

  return (
    <Card className="bg-neutral-800/40 border-neutral-700">
      <CardHeader className="pb-4">
        <CardTitle className="text-white flex items-center gap-3 text-lg">
          <Settings className="w-5 h-5 text-emerald-400" />
          Compression Control
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-6">
        
        {/* Compression Profiles */}
        <div>
          <label className="text-sm font-medium text-neutral-300 mb-3 block">
            Compression Profiles
          </label>
          <div className="grid grid-cols-2 gap-3">
            {Object.entries(COMPRESSION_PROFILES).map(([key, profile]) => {
              const Icon = profile.icon;
              const isActive = settings.preset === key;
              
              return (
                <Button
                  key={key}
                  variant="ghost"
                  disabled={disabled}
                  onClick={() => handleProfileChange(key)}
                  className={`h-20 p-4 flex-col gap-2 transition-all border ${
                    isActive 
                      ? `bg-gradient-to-br ${profile.color} text-white border-transparent shadow-lg` 
                      : 'bg-neutral-700/30 text-neutral-300 border-neutral-600 hover:bg-neutral-600/40'
                  }`}
                >
                  <Icon className="w-6 h-6" />
                  <div className="text-center">
                    <div className="text-sm font-semibold">{profile.name}</div>
                    <div className={`text-xs mt-1 ${isActive ? 'text-white/80' : 'text-neutral-500'}`}>
                      Level {profile.level}
                    </div>
                  </div>
                </Button>
              );
            })}
          </div>
          
          <div className="mt-3 p-3 bg-neutral-700/20 rounded-lg border border-neutral-600/50">
            <p className="text-xs text-neutral-400">{currentProfile.desc}</p>
          </div>
        </div>

        {/* Advanced Settings */}
        <div className="space-y-4">
          <div className="flex items-center gap-2">
            <Sliders className="w-4 h-4 text-neutral-400" />
            <label className="text-sm font-medium text-neutral-300">Advanced Settings</label>
          </div>
          
          {/* Compression Level */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <span className="text-xs text-neutral-400">Compression Level</span>
              <Badge className="bg-neutral-700 text-teal-400 border-neutral-600">
                {settings.compressionLevel}
              </Badge>
            </div>
            <Slider
              value={[settings.compressionLevel]}
              onValueChange={(value) => updateSetting('compressionLevel', value[0])}
              max={22}
              min={-7}
              step={1}
              disabled={disabled}
              className="mb-2"
            />
            <div className="flex justify-between text-[10px] text-neutral-500">
              <span>Fast</span>
              <span>Balanced</span>
              <span>Maximum</span>
            </div>
          </div>

          {/* Memory Budget Control */}
          <div>
            <label className="text-xs text-neutral-400 mb-2 block">
              Memory Usage: {settings.memoryBudget === 0 ? 'Automatic' : `${settings.memoryBudget}% RAM`}
            </label>
            <Slider
              value={[settings.memoryBudget]}
              onValueChange={(value) => updateSetting('memoryBudget', value[0])}
              max={100}
              min={0}
              step={10}
              disabled={disabled}
              className="mb-2"
            />
            <div className="flex justify-between text-[10px] text-neutral-500">
              <span>Auto</span>
              <span>50%</span>
              <span>100%</span>
            </div>
          </div>



          {/* Thread Control */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-neutral-400 mb-1 block">CPU Threads</label>
              <div className="flex items-center gap-2">
                <Cpu className="w-3 h-3 text-neutral-500" />
                <Input
                  type="number"
                  min="0"
                  max="32"
                  value={settings.threads}
                  onChange={(e) => updateSetting('threads', parseInt(e.target.value) || 0)}
                  disabled={disabled}
                  className="h-8 text-xs bg-neutral-700/30 border-neutral-600 text-white"
                  placeholder="Auto"
                />
              </div>
            </div>
            
            <div>
              <label className="text-xs text-neutral-400 mb-1 block">Codec Threads</label>
              <div className="flex items-center gap-2">
                <Cpu className="w-3 h-3 text-neutral-500" />
                <Input
                  type="number"
                  min="0"
                  max="16"
                  value={settings.codecThreads}
                  onChange={(e) => updateSetting('codecThreads', parseInt(e.target.value) || 0)}
                  disabled={disabled}
                  className="h-8 text-xs bg-neutral-700/30 border-neutral-600 text-white"
                  placeholder="Auto"
                />
              </div>
            </div>
          </div>


        </div>

        {/* Security */}
        <div>
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <Shield className="w-4 h-4 text-neutral-400" />
              <label className="text-sm font-medium text-neutral-300">Encryption</label>
            </div>
            <Switch
              checked={settings.useEncryption}
              onCheckedChange={(checked) => updateSetting('useEncryption', checked)}
              disabled={disabled}
            />
          </div>
          
          {settings.useEncryption && (
            <div className="relative">
              <Lock className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400 w-4 h-4" />
              <Input
                type="password"
                placeholder="Enter secure password"
                value={settings.password}
                onChange={(e) => updateSetting('password', e.target.value)}
                disabled={disabled}
                className="pl-10 bg-neutral-700/30 border-neutral-600 text-white placeholder-neutral-500"
              />
              <div className="mt-2 text-xs text-neutral-500">
                AES-256-GCM encryption will be applied
              </div>
            </div>
          )}
        </div>

        {/* Status Summary */}
        <div className={`p-4 rounded-lg bg-gradient-to-r ${currentProfile.color.replace('from-', 'from-').replace('to-', 'to-')}/10 border border-current/20`}>
          <div className="flex items-center gap-2 mb-2">
            <currentProfile.icon className="w-4 h-4" />
            <span className="text-sm font-semibold text-white">
              {currentProfile.name}{isModified ? ' (Modified)' : ''} Profile Active
            </span>
          </div>
          <div className="text-xs text-neutral-300 space-y-1">
            <div>Compression: Level {settings.compressionLevel}</div>
            <div>Security: {settings.useEncryption ? 'AES-256 Encrypted' : 'Unencrypted'}</div>
            <div>Threads: {settings.threads || 'Auto'} CPU, {settings.codecThreads || 'Auto'} Codec</div>
            <div>Memory: {settings.memoryBudget === 0 ? 'Automatic' : `${settings.memoryBudget}% RAM`}</div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}