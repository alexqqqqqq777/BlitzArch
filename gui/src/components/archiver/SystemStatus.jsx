import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Activity, 
  Archive, 
  Shield, 
  Clock,
  TrendingUp,
  Database,
  CheckCircle,
  AlertTriangle,
  Trash2,
  Info,
  AlertCircle,
  XCircle
} from 'lucide-react';

export default function SystemStatus({ archives, logs, isProcessing, onDeleteArchive }) {
  const totalSize = archives.reduce((sum, archive) => sum + (archive.size || 0), 0);
  const encryptedCount = archives.filter(archive => archive.encrypted).length;

  const formatSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  const getStatusColor = () => {
    if (isProcessing) return 'text-cyan-400';
    if (archives.length === 0) return 'text-neutral-400';
    return 'text-emerald-400';
  };

  const getLogIcon = (type) => {
    switch (type) {
      case 'success': return <CheckCircle className="w-4 h-4 text-emerald-400" />;
      case 'error': return <XCircle className="w-4 h-4 text-red-400" />;
      case 'warning': return <AlertTriangle className="w-4 h-4 text-yellow-400" />;
      default: return <Info className="w-4 h-4 text-cyan-400" />;
    }
  };

  return (
    <div className="space-y-6">
      {/* System Status */}
      <Card className="bg-neutral-800/40 border-neutral-700">
        <CardHeader className="pb-4">
          <CardTitle className="text-white flex items-center gap-3">
            <Activity className="w-5 h-5 text-purple-400" />
            System Status
          </CardTitle>
        </CardHeader>
        
        <CardContent className="space-y-4">
          
          {/* Status Indicator */}
          <div className={`flex items-center gap-3 p-3 rounded-lg ${
            isProcessing 
              ? 'bg-cyan-500/10 border border-cyan-500/20' 
              : 'bg-neutral-700/30'
          }`}>
            <motion.div
              animate={{ 
                scale: isProcessing ? [1, 1.2, 1] : 1,
                rotate: isProcessing ? 360 : 0
              }}
              transition={{ 
                scale: { duration: 1, repeat: isProcessing ? Infinity : 0 },
                rotate: { duration: 2, repeat: isProcessing ? Infinity : 0, ease: "linear" }
              }}
            >
              {isProcessing ? (
                <Activity className="w-5 h-5 text-cyan-400" />
              ) : (
                <CheckCircle className="w-5 h-5 text-emerald-400" />
              )}
            </motion.div>
            <div>
              <div className={`text-sm font-semibold ${getStatusColor()}`}>
                {isProcessing ? 'Processing Active' : 'Engine Ready'}
              </div>
              <div className="text-xs text-neutral-400">
                {isProcessing ? 'Archive operation in progress' : 'BlitzArch engine online'}
              </div>
            </div>
          </div>

          {/* Archive Statistics */}
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 bg-neutral-700/30 rounded-lg">
              <div className="flex items-center gap-2 mb-2">
                <Archive className="w-4 h-4 text-cyan-400" />
                <span className="text-xs text-neutral-400">Total Archives</span>
              </div>
              <div className="text-lg font-bold text-white">{archives.length}</div>
            </div>
            
            <div className="p-3 bg-neutral-700/30 rounded-lg">
              <div className="flex items-center gap-2 mb-2">
                <Database className="w-4 h-4 text-emerald-400" />
                <span className="text-xs text-neutral-400">Total Size</span>
              </div>
              <div className="text-lg font-bold text-white">{formatSize(totalSize)}</div>
            </div>
          </div>

          {/* Recent Archives */}
          <div>
            <div className="flex items-center gap-2 mb-3">
              <Clock className="w-4 h-4 text-neutral-400" />
              <span className="text-sm font-medium text-neutral-300">Recent Archives</span>
            </div>
            
            <div className="space-y-2 max-h-32 overflow-y-auto">
              {archives.slice(0, 4).map((archive, index) => (
                <motion.div
                  key={index}
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: index * 0.1 }}
                  className="flex items-center justify-between p-2 bg-neutral-700/20 rounded-lg group"
                >
                  <div className="flex items-center gap-2 flex-1 min-w-0">
                    <Archive className="w-3 h-3 text-cyan-400 flex-shrink-0" />
                    <span className="text-xs text-neutral-300 truncate">
                      {archive.name}
                    </span>
                    {archive.encrypted && (
                      <Shield className="w-3 h-3 text-orange-400 flex-shrink-0" />
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge className="bg-neutral-600 text-neutral-300 text-[10px] px-1 py-0">
                      {archive.preset}
                    </Badge>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => onDeleteArchive(archive.path)}
                      className="opacity-0 group-hover:opacity-100 transition-opacity p-1 h-auto text-red-400 hover:text-red-300"
                    >
                      <Trash2 className="w-3 h-3" />
                    </Button>
                  </div>
                </motion.div>
              ))}
              
              {archives.length === 0 && (
                <div className="text-center py-6 text-neutral-500">
                  <Archive className="w-8 h-8 mx-auto mb-2 opacity-50" />
                  <p className="text-xs">No archives created yet</p>
                </div>
              )}
            </div>
          </div>

          {/* Engine Status */}
          <div className="p-3 bg-gradient-to-r from-emerald-500/10 to-cyan-500/10 rounded-lg border border-emerald-500/20">
            <div className="flex items-center gap-2 mb-2">
              <CheckCircle className="w-4 h-4 text-emerald-400" />
              <span className="text-sm font-medium text-emerald-300">Engine Status</span>
            </div>
            <div className="text-xs text-emerald-400">
              Rust backend online • Ready for operations
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Activity Log */}
      <Card className="bg-neutral-800/40 border-neutral-700">
        <CardHeader className="pb-4">
          <CardTitle className="text-white flex items-center gap-3">
            <AlertCircle className="w-5 h-5 text-yellow-400" />
            Activity Log
          </CardTitle>
        </CardHeader>
        
        <CardContent>
          <div className="max-h-64 overflow-y-auto space-y-2">
            <AnimatePresence>
              {logs.map((log, index) => (
                <motion.div
                  key={index}
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 20 }}
                  className="flex items-start gap-3 p-2 rounded-lg bg-neutral-700/20"
                >
                  {getLogIcon(log.type)}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs text-neutral-500">{log.timestamp}</span>
                    </div>
                    <span className={`text-sm ${
                      log.type === 'success' ? 'text-emerald-400' :
                      log.type === 'error' ? 'text-red-400' :
                      log.type === 'warning' ? 'text-yellow-400' :
                      'text-cyan-400'
                    }`}>
                      {log.message}
                    </span>
                  </div>
                </motion.div>
              ))}
            </AnimatePresence>
            
            {logs.length === 0 && (
              <div className="text-center text-neutral-500 py-8">
                <Activity className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p className="text-sm">No activity logged yet</p>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}