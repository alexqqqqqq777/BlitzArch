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
  Shuffle,
  Filter,
  FolderOpen
} from 'lucide-react';

export default function ArchiveExplorer({ archive, onExtract, onClose }) {
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
      .slice(0, Math.min(6, filteredFiles.length))
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
      case 'webp':
        return <Image className="w-4 h-4 text-teal-400" />;
      case 'mp4':
      case 'avi':
      case 'mov':
      case 'mkv':
        return <Film className="w-4 h-4 text-purple-400" />;
      case 'txt':
      case 'md':
      case 'doc':
      case 'docx':
      case 'pdf':
        return <FileText className="w-4 h-4 text-cyan-400" />;
      default:
        return <File className="w-4 h-4 text-neutral-400" />;
    }
  };

  const formatSize = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  return (
    <div className="space-y-6">
      
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <FolderOpen className="w-6 h-6 text-teal-400" />
          <div>
            <h3 className="text-xl font-bold text-white">{archive.name}</h3>
            <p className="text-neutral-400 text-sm">
              {archive.files.length} files • {formatSize(archive.size)}
            </p>
          </div>
        </div>
        <Button
          variant="outline"
          size="icon"
          onClick={onClose}
          className="border-neutral-600 text-neutral-400 hover:text-white hover:bg-neutral-700"
        >
          <X className="w-4 h-4" />
        </Button>
      </div>

      {/* Search and Filters */}
      <div className="space-y-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400 w-4 h-4" />
          <Input
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Search files in archive..."
            className="pl-10 bg-neutral-700/30 border-neutral-600 text-white placeholder-neutral-500"
          />
        </div>

        <div className="flex items-center gap-2 flex-wrap">
          <Filter className="w-4 h-4 text-neutral-400" />
          {extensions.map(ext => (
            <Badge
              key={ext}
              variant={selectedExtensions.includes(ext) ? "default" : "outline"}
              className={`cursor-pointer transition-all text-xs ${
                selectedExtensions.includes(ext) 
                  ? 'bg-gradient-to-r from-teal-500 to-cyan-500 text-white border-0 hover:from-teal-400 hover:to-cyan-400' 
                  : 'bg-neutral-700/30 text-neutral-300 border-neutral-600 hover:bg-neutral-600/40'
              }`}
              onClick={() => handleExtensionToggle(ext)}
            >
              {ext === 'all' ? 'All Types' : ext}
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
          className="border-neutral-600 text-neutral-300 hover:bg-neutral-700"
        >
          {selectedFiles.length === filteredFiles.length ? 'Deselect All' : 'Select All'}
        </Button>
        
        <Button
          variant="outline"
          size="sm"
          onClick={handleRandomSelect}
          className="border-neutral-600 text-neutral-300 hover:bg-neutral-700"
        >
          <Shuffle className="w-4 h-4 mr-2" />
          Random Sample
        </Button>
        
        <Button
          size="sm"
          onClick={() => onExtract(selectedFiles)}
          disabled={selectedFiles.length === 0}
          className="bg-gradient-to-r from-teal-500 to-cyan-500 hover:from-teal-400 hover:to-cyan-400 text-white border-0"
        >
          <Download className="w-4 h-4 mr-2" />
          Extract Selected ({selectedFiles.length})
        </Button>
      </div>

      {/* Files List */}
      <div className="bg-neutral-800/30 rounded-xl border border-neutral-700">
        <div className="max-h-80 overflow-y-auto">
          <div className="p-4 space-y-1">
            {filteredFiles.map((file, index) => (
              <div
                key={index}
                className={`flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-all ${
                  selectedFiles.includes(file.path) 
                    ? 'bg-gradient-to-r from-teal-500/20 to-cyan-500/20 border border-teal-500/30' 
                    : 'hover:bg-neutral-700/30'
                }`}
                onClick={() => handleFileSelect(file.path)}
              >
                <Checkbox
                  checked={selectedFiles.includes(file.path)}
                  className="data-[state=checked]:bg-teal-500 data-[state=checked]:border-teal-500"
                />
                
                {getFileIcon(file.name)}
                
                <div className="flex-1 min-w-0">
                  <p className="text-white text-sm font-medium truncate">{file.name}</p>
                  <div className="flex items-center gap-4 text-xs text-neutral-400">
                    <span>{formatSize(file.size)}</span>
                    <span>CRC: {file.crc32?.slice(0, 8)}</span>
                    <span className="text-neutral-500">•</span>
                    <span className="truncate">{file.path}</span>
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  {file.crc_ok ? (
                    <div className="flex items-center gap-1">
                      <CheckCircle className="w-4 h-4 text-emerald-400" />
                      <span className="text-xs text-emerald-400">Valid</span>
                    </div>
                  ) : (
                    <div className="flex items-center gap-1">
                      <XCircle className="w-4 h-4 text-red-400" />
                      <span className="text-xs text-red-400">Error</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
            
            {filteredFiles.length === 0 && (
              <div className="text-center py-12 text-neutral-500">
                <File className="w-12 h-12 mx-auto mb-3 opacity-50" />
                <p>No files match your search criteria</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}