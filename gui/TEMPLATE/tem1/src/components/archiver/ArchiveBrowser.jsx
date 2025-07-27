import React, { useState, useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Checkbox } from '@/components/ui/checkbox';
import { 
  Search, 
  Filter, 
  FileText, 
  Image, 
  Film, 
  File,
  Folder,
  CheckCircle,
  XCircle,
  Download,
  Shuffle
} from 'lucide-react';

export default function ArchiveBrowser({ archive, onExtractFiles }) {
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedExtensions, setSelectedExtensions] = useState(['all']);
  const [selectedFiles, setSelectedFiles] = useState([]);

  const extensions = useMemo(() => {
    if (!archive?.files) return [];
    const exts = [...new Set(archive.files.map(file => {
      const ext = file.name.split('.').pop();
      return ext ? `.${ext}` : '';
    }).filter(Boolean))];
    return ['all', ...exts];
  }, [archive?.files]);

  const filteredFiles = useMemo(() => {
    if (!archive?.files) return [];
    
    return archive.files.filter(file => {
      const matchesSearch = file.name.toLowerCase().includes(searchTerm.toLowerCase());
      const matchesExtension = selectedExtensions.includes('all') || 
        selectedExtensions.some(ext => file.name.endsWith(ext));
      return matchesSearch && matchesExtension;
    });
  }, [archive?.files, searchTerm, selectedExtensions]);

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

  const handleFileSelect = (file) => {
    setSelectedFiles(prev => 
      prev.includes(file.path) 
        ? prev.filter(path => path !== file.path)
        : [...prev, file.path]
    );
  };

  const handleSelectAll = () => {
    if (selectedFiles.length === filteredFiles.length) {
      setSelectedFiles([]);
    } else {
      setSelectedFiles(filteredFiles.map(file => file.path));
    }
  };

  const handleRandomExtract = () => {
    const randomFiles = filteredFiles
      .sort(() => 0.5 - Math.random())
      .slice(0, Math.min(5, filteredFiles.length));
    
    if (onExtractFiles) {
      onExtractFiles(randomFiles);
    }
  };

  const getFileIcon = (fileName) => {
    const ext = fileName.split('.').pop()?.toLowerCase();
    switch (ext) {
      case 'jpg':
      case 'jpeg':
      case 'png':
      case 'gif':
        return <Image className="w-4 h-4 text-blue-400" />;
      case 'mp4':
      case 'avi':
      case 'mov':
        return <Film className="w-4 h-4 text-purple-400" />;
      case 'txt':
      case 'md':
      case 'doc':
      case 'docx':
        return <FileText className="w-4 h-4 text-green-400" />;
      default:
        return <File className="w-4 h-4 text-gray-400" />;
    }
  };

  const formatSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  if (!archive) {
    return (
      <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
        <CardContent className="p-8 text-center">
          <Folder className="w-16 h-16 text-slate-500 mx-auto mb-4" />
          <p className="text-slate-400">Загрузите архив для просмотра содержимого</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
      <CardHeader className="pb-4">
        <CardTitle className="text-cyan-300 flex items-center gap-2">
          <Folder className="w-5 h-5" />
          {archive.name}
        </CardTitle>
        <p className="text-slate-400 text-sm">
          {archive.files?.length || 0} файлов • {formatSize(archive.size)}
        </p>
      </CardHeader>
      
      <CardContent className="space-y-4">
        {/* Search and Filters */}
        <div className="space-y-3">
          <div className="relative">
            <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-400 w-4 h-4" />
            <Input
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Поиск файлов..."
              className="pl-10 bg-slate-700/50 border-slate-600 text-slate-200"
            />
          </div>

          <div className="flex flex-wrap gap-2">
            {extensions.map(ext => (
              <Badge
                key={ext}
                variant={selectedExtensions.includes(ext) ? "default" : "secondary"}
                className={`cursor-pointer transition-all ${
                  selectedExtensions.includes(ext) 
                    ? 'bg-cyan-500 text-black hover:bg-cyan-400' 
                    : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                }`}
                onClick={() => handleExtensionToggle(ext)}
              >
                {ext === 'all' ? 'Все' : ext}
              </Badge>
            ))}
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-2 flex-wrap">
          <Button
            variant="outline"
            size="sm"
            onClick={handleSelectAll}
            className="border-slate-600 text-slate-200 hover:bg-slate-700"
          >
            {selectedFiles.length === filteredFiles.length ? 'Снять все' : 'Выбрать все'}
          </Button>
          
          <Button
            variant="outline"
            size="sm"
            onClick={handleRandomExtract}
            className="border-slate-600 text-slate-200 hover:bg-slate-700"
          >
            <Shuffle className="w-4 h-4 mr-2" />
            Случайно
          </Button>
          
          <Button
            size="sm"
            onClick={() => onExtractFiles && onExtractFiles(selectedFiles)}
            disabled={selectedFiles.length === 0}
            className="bg-cyan-500 hover:bg-cyan-400 text-black"
          >
            <Download className="w-4 h-4 mr-2" />
            Извлечь ({selectedFiles.length})
          </Button>
        </div>

        {/* Files Table */}
        <div className="max-h-64 overflow-y-auto">
          <div className="space-y-1">
            {filteredFiles.map((file, index) => (
              <div
                key={index}
                className={`flex items-center gap-3 p-2 rounded-lg cursor-pointer transition-all ${
                  selectedFiles.includes(file.path) 
                    ? 'bg-cyan-500/20 border border-cyan-500/30' 
                    : 'hover:bg-slate-700/30'
                }`}
                onClick={() => handleFileSelect(file)}
              >
                <Checkbox
                  checked={selectedFiles.includes(file.path)}
                  onChange={() => handleFileSelect(file)}
                  className="data-[state=checked]:bg-cyan-500 data-[state=checked]:border-cyan-500"
                />
                
                {getFileIcon(file.name)}
                
                <div className="flex-1 min-w-0">
                  <p className="text-slate-200 text-sm truncate">{file.name}</p>
                  <p className="text-slate-500 text-xs">
                    {formatSize(file.size)} • CRC: {file.crc32?.slice(0, 8)}
                  </p>
                </div>
                
                <div className="flex items-center gap-2">
                  {file.crc_ok ? (
                    <CheckCircle className="w-4 h-4 text-green-400" />
                  ) : (
                    <XCircle className="w-4 h-4 text-red-400" />
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}