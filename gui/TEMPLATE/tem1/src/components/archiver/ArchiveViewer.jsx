import React, { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { 
  Search, 
  Download, 
  X, 
  File, 
  Image, 
  FileText, 
  Film,
  CheckCircle,
  XCircle,
  Shuffle
} from 'lucide-react';

export default function ArchiveViewer({ archive, onExtract, onClose }) {
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedFiles, setSelectedFiles] = useState([]);
  const [selectedExtensions, setSelectedExtensions] = useState(['all']);

  const extensions = useMemo(() => {
    const exts = [...new Set(archive.files.map(file => {
      const ext = file.name.split('.').pop();
      return ext ? `.${ext}` : '';
    }).filter(Boolean))];
    return ['all', ...exts];
  }, [archive.files]);

  const filteredFiles = useMemo(() => {
    return archive.files.filter(file => {
      const matchesSearch = file.name.toLowerCase().includes(searchTerm.toLowerCase());
      const matchesExtension = selectedExtensions.includes('all') || 
        selectedExtensions.some(ext => file.name.endsWith(ext));
      return matchesSearch && matchesExtension;
    });
  }, [archive.files, searchTerm, selectedExtensions]);

  const handleExtensionToggle = (extension) => {
    if (extension === 'all') {
      setSelectedExtensions(['all']);
    } else {
      setSelectedExtensions(prev => {
        const newExts = prev.includes(extension) 
          ? prev.filter(ext => ext !== extension)
          : [...prev.filter(ext => ext !== 'all'), extension];
        return newExts.length === 0 ? ['all'] : newExts;
      });
    }
  };

  const handleFileSelect = (filePath) => {
    setSelectedFiles(prev => 
      prev.includes(filePath) 
        ? prev.filter(path => path !== filePath)
        : [...prev, filePath]
    );
  };

  const handleSelectAll = () => {
    if (selectedFiles.length === filteredFiles.length) {
      setSelectedFiles([]);
    } else {
      setSelectedFiles(filteredFiles.map(file => file.path));
    }
  };

  const handleRandomSelect = () => {
    const randomFiles = filteredFiles
      .sort(() => 0.5 - Math.random())
      .slice(0, Math.min(5, filteredFiles.length))
      .map(file => file.path);
    setSelectedFiles(randomFiles);
  };

  const getFileIcon = (fileName) => {
    const ext = fileName.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'jpg':
      case 'jpeg':
      case 'png':
      case 'gif':
        return <Image className="w-4 h-4 text-amber-400" />;
      case 'mp4':
      case 'avi':
      case 'mov':
        return <Film className="w-4 h-4 text-violet-400" />;
      case 'txt':
      case 'md':
      case 'doc':
      case 'docx':
        return <FileText className="w-4 h-4 text-emerald-400" />;
      default:
        return <File className="w-4 h-4 text-slate-400" />;
    }
  };

  const formatSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="space-y-6">
      
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-xl font-bold text-white">{archive.name}</h3>
          <p className="text-slate-400">
            {archive.files.length} файлов • {formatSize(archive.size)}
          </p>
        </div>
        <Button
          variant="outline"
          size="icon"
          onClick={onClose}
          className="border-slate-600/50 text-slate-400 hover:text-white hover:bg-slate-700/50"
        >
          <X className="w-4 h-4" />
        </Button>
      </div>

      {/* Search and Filters */}
      <div className="space-y-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-400 w-4 h-4" />
          <Input
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Поиск файлов..."
            className="pl-10 bg-slate-800/30 border-slate-600/50 text-white placeholder-slate-400"
          />
        </div>

        <div className="flex flex-wrap gap-2">
          {extensions.map(ext => (
            <Badge
              key={ext}
              variant={selectedExtensions.includes(ext) ? "default" : "outline"}
              className={`cursor-pointer transition-all ${
                selectedExtensions.includes(ext) 
                  ? 'bg-gradient-to-r from-amber-500 to-orange-500 text-white hover:from-amber-400 hover:to-orange-400' 
                  : 'bg-slate-800/30 text-slate-300 border-slate-600/50 hover:bg-slate-700/40'
              }`}
              onClick={() => handleExtensionToggle(ext)}
            >
              {ext === 'all' ? 'Все' : ext}
            </Badge>
          ))}
        </div>
      </div>

      {/* Actions */}
      <div className="flex gap-3 flex-wrap">
        <Button
          variant="outline"
          size="sm"
          onClick={handleSelectAll}
          className="border-slate-600/50 text-slate-300 hover:bg-slate-700/40"
        >
          {selectedFiles.length === filteredFiles.length ? 'Снять все' : 'Выбрать все'}
        </Button>
        
        <Button
          variant="outline"
          size="sm"
          onClick={handleRandomSelect}
          className="border-slate-600/50 text-slate-300 hover:bg-slate-700/40"
        >
          <Shuffle className="w-4 h-4 mr-2" />
          Случайно
        </Button>
        
        <Button
          size="sm"
          onClick={() => onExtract(selectedFiles)}
          disabled={selectedFiles.length === 0}
          className="bg-gradient-to-r from-violet-500 to-purple-500 hover:from-violet-400 hover:to-purple-400 text-white"
        >
          <Download className="w-4 h-4 mr-2" />
          Извлечь ({selectedFiles.length})
        </Button>
      </div>

      {/* Files List */}
      <div className="max-h-96 overflow-y-auto space-y-1 bg-slate-800/20 rounded-xl p-4">
        {filteredFiles.map((file, index) => (
          <div
            key={index}
            className={`flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-all ${
              selectedFiles.includes(file.path) 
                ? 'bg-gradient-to-r from-amber-500/20 to-orange-500/20 border border-amber-500/30' 
                : 'hover:bg-slate-700/30'
            }`}
            onClick={() => handleFileSelect(file.path)}
          >
            <Checkbox
              checked={selectedFiles.includes(file.path)}
              className="data-[state=checked]:bg-amber-500 data-[state=checked]:border-amber-500"
            />
            
            {getFileIcon(file.name)}
            
            <div className="flex-1 min-w-0">
              <p className="text-white text-sm font-medium truncate">{file.name}</p>
              <p className="text-slate-400 text-xs">
                {formatSize(file.size)} • CRC: {file.crc32?.slice(0, 8)}
              </p>
            </div>
            
            <div className="flex items-center gap-2">
              {file.crc_ok ? (
                <CheckCircle className="w-4 h-4 text-emerald-400" />
              ) : (
                <XCircle className="w-4 h-4 text-red-400" />
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}