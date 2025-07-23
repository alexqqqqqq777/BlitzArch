import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Activity, 
  Archive, 
  CheckCircle, 
  XCircle, 
  AlertCircle,
  Database,
  Clock
} from 'lucide-react';

export default function StatusBar({ logs, archives }) {
  const getLogIcon = (type) => {
    switch (type) {
      case 'success': return <CheckCircle className="w-4 h-4 text-green-400" />;
      case 'error': return <XCircle className="w-4 h-4 text-red-400" />;
      case 'warning': return <AlertCircle className="w-4 h-4 text-yellow-400" />;
      default: return <Activity className="w-4 h-4 text-blue-400" />;
    }
  };

  const getLogColor = (type) => {
    switch (type) {
      case 'success': return 'text-green-400';
      case 'error': return 'text-red-400';
      case 'warning': return 'text-yellow-400';
      default: return 'text-blue-400';
    }
  };

  const totalSize = archives.reduce((sum, archive) => sum + (archive.size || 0), 0);
  const totalFiles = archives.reduce((sum, archive) => sum + (archive.files_count || 0), 0);

  const formatSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="grid md:grid-cols-2 gap-6 mt-8">
      {/* Statistics */}
      <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
        <CardHeader className="pb-4">
          <CardTitle className="text-cyan-300 flex items-center gap-2">
            <Database className="w-5 h-5" />
            Статистика
          </CardTitle>
        </CardHeader>
        
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-700/30">
              <Archive className="w-4 h-4 text-purple-400" />
              <div>
                <div className="text-sm font-medium text-slate-200">
                  {archives.length}
                </div>
                <div className="text-xs text-slate-500">Архивов</div>
              </div>
            </div>
            
            <div className="flex items-center gap-2 p-3 rounded-lg bg-slate-700/30">
              <Database className="w-4 h-4 text-blue-400" />
              <div>
                <div className="text-sm font-medium text-slate-200">
                  {formatSize(totalSize)}
                </div>
                <div className="text-xs text-slate-500">Общий размер</div>
              </div>
            </div>
          </div>

          {/* Recent Archives */}
          <div className="space-y-2">
            <h4 className="text-sm font-medium text-slate-300">Последние архивы</h4>
            <div className="max-h-32 overflow-y-auto space-y-1">
              {archives.slice(0, 5).map((archive, index) => (
                <div key={index} className="flex items-center gap-2 p-2 rounded bg-slate-700/20">
                  <Archive className="w-3 h-3 text-cyan-400" />
                  <span className="text-xs text-slate-300 truncate flex-1">
                    {archive.name}
                  </span>
                  <Badge variant="outline" className="text-xs border-slate-600 text-slate-400">
                    {archive.preset}
                  </Badge>
                </div>
              ))}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Activity Log */}
      <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
        <CardHeader className="pb-4">
          <CardTitle className="text-cyan-300 flex items-center gap-2">
            <Activity className="w-5 h-5" />
            Журнал активности
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
                  className="flex items-center gap-3 p-2 rounded-lg bg-slate-700/20"
                >
                  <div className="flex items-center gap-2">
                    {getLogIcon(log.type)}
                    <Clock className="w-3 h-3 text-slate-500" />
                    <span className="text-xs text-slate-500">{log.timestamp}</span>
                  </div>
                  <span className={`text-sm flex-1 ${getLogColor(log.type)}`}>
                    {log.message}
                  </span>
                </motion.div>
              ))}
            </AnimatePresence>
            
            {logs.length === 0 && (
              <div className="text-center text-slate-500 py-8">
                <Activity className="w-8 h-8 mx-auto mb-2 opacity-50" />
                <p className="text-sm">Журнал активности пуст</p>
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}