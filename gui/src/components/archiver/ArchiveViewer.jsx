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
  Folder,
  ChevronRight,
  ChevronLeft
} from 'lucide-react';

export default function ArchiveViewer({ archive, onExtract, onClose }) {
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedFiles, setSelectedFiles] = useState([]);
  const [selectedExtensions, setSelectedExtensions] = useState(['all']);
  const [currentPath, setCurrentPath] = useState('/');
  const [currentPage, setCurrentPage] = useState(1);
  const itemsPerPage = 50; // Пагинация

  // Build folder hierarchy and current level items
  const { displayedFiles, totalItems, folders, breadcrumb, extensions } = useMemo(() => {
    if (!archive?.files || archive.files.length === 0) {
      return { displayedFiles: [], totalItems: 0, folders: new Set(), breadcrumb: [], extensions: ['all'] };
    }
    
    // Build folder structure from file paths
    const folderStructure = new Map();
    const allFolders = new Set();
    
    archive.files.forEach(file => {
      const pathParts = file.path.split('/').filter(p => p);
      let currentLevel = folderStructure;
      
      // Build nested folder structure
      for (let i = 0; i < pathParts.length - 1; i++) {
        const folderName = pathParts[i];
        allFolders.add('/' + pathParts.slice(0, i + 1).join('/'));
        
        if (!currentLevel.has(folderName)) {
          currentLevel.set(folderName, { folders: new Map(), files: [] });
        }
        currentLevel = currentLevel.get(folderName).folders;
      }
      
      // Add file to its parent folder
      const fileName = pathParts[pathParts.length - 1];
      if (pathParts.length > 1) {
        const parentPath = pathParts.slice(0, -1);
        let parentLevel = folderStructure;
        for (const folder of parentPath) {
          if (parentLevel.has(folder)) {
            parentLevel = parentLevel.get(folder).folders;
          }
        }
        // Find correct parent and add file
        let targetLevel = folderStructure;
        for (let i = 0; i < parentPath.length - 1; i++) {
          targetLevel = targetLevel.get(parentPath[i])?.folders;
        }
        if (targetLevel?.get(parentPath[parentPath.length - 1])) {
          targetLevel.get(parentPath[parentPath.length - 1]).files.push(file);
        }
      } else {
        // Root level file
        if (!folderStructure.has('__root_files__')) {
          folderStructure.set('__root_files__', { folders: new Map(), files: [] });
        }
        folderStructure.get('__root_files__').files.push(file);
      }
    });
    
    // Generate breadcrumb navigation
    const pathParts = currentPath === '/' ? [] : currentPath.split('/').filter(p => p);
    const breadcrumb = [
      { name: 'Root', path: '/' },
      ...pathParts.map((part, index) => ({
        name: part,
        path: '/' + pathParts.slice(0, index + 1).join('/')
      }))
    ];
    
    // Get items for current path
    let currentItems = [];
    
    if (currentPath === '/') {
      // Root level: show folders and root files
      for (const [folderName, folderData] of folderStructure) {
        if (folderName !== '__root_files__') {
          currentItems.push({
            name: folderName,
            path: '/' + folderName,
            size: folderData.files.reduce((sum, f) => sum + (f.size || 0), 0),
            is_dir: true,
            type: 'folder'
          });
        }
      }
      
      // Add root files
      if (folderStructure.has('__root_files__')) {
        currentItems = currentItems.concat(folderStructure.get('__root_files__').files);
      }
    } else {
      // Navigate to specific folder
      const pathParts = currentPath.split('/').filter(p => p);
      let currentLevel = folderStructure;
      
      // Find current folder
      let targetFolderData = null;
      if (pathParts.length === 1) {
        targetFolderData = folderStructure.get(pathParts[0]);
      } else {
        let level = folderStructure;
        for (const part of pathParts) {
          if (level.has(part)) {
            targetFolderData = level.get(part);
            level = targetFolderData.folders;
          }
        }
      }
      
      if (targetFolderData) {
        // Add subfolders
        for (const [folderName, folderData] of targetFolderData.folders) {
          currentItems.push({
            name: folderName,
            path: currentPath.endsWith('/') ? currentPath + folderName : currentPath + '/' + folderName,
            size: folderData.files.reduce((sum, f) => sum + (f.size || 0), 0),
            is_dir: true,
            type: 'folder'
          });
        }
        
        // Add files in current folder
        currentItems = currentItems.concat(targetFolderData.files || []);
      }
    }
    
    // Get extensions from current items
    const exts = [...new Set(currentItems.filter(item => !item.is_dir).map(file => {
      const ext = file.name.split('.').pop();
      return ext ? `.${ext}` : '';
    }).filter(Boolean))];
    const extensions = ['all', ...exts];
    
    return { 
      displayedFiles: currentItems, 
      totalItems: currentItems.length, 
      folders: allFolders, 
      breadcrumb,
      extensions 
    };
  }, [archive?.files, currentPath]);

  // Files matching current filters across the ENTIRE archive (рекурсивно)
  const globalFilteredFiles = useMemo(() => {
    if (!archive?.files) return [];
    const lowerSearch = searchTerm.toLowerCase();
    return archive.files.filter(item => {
      if (item.is_dir) return false;
      const matchesSearch = item.name.toLowerCase().includes(lowerSearch) || item.path.toLowerCase().includes(lowerSearch);
      const matchesExtension = selectedExtensions.includes('all') || selectedExtensions.some(ext => item.name.endsWith(ext));
      return matchesSearch && matchesExtension;
    });
  }, [archive?.files, searchTerm, selectedExtensions]);

  // Files that are currently visible in the UI (текущая папка + пагинация)
  const filteredFiles = useMemo(() => {
    let filtered = displayedFiles.filter(item => {
      const lowerSearch = searchTerm.toLowerCase();
      const matchesSearch = item.name.toLowerCase().includes(lowerSearch) || item.path.toLowerCase().includes(lowerSearch);
      const matchesExtension = item.is_dir || selectedExtensions.includes('all') || 
        selectedExtensions.some(ext => item.name.endsWith(ext));
      return matchesSearch && matchesExtension;
    });
    
    // Sort: folders first
    filtered.sort((a, b) => {
      if (a.is_dir && !b.is_dir) return -1;
      if (!a.is_dir && b.is_dir) return 1;
      return a.name.localeCompare(b.name);
    });
    
    // Apply pagination
    const startIndex = (currentPage - 1) * itemsPerPage;
    return filtered.slice(startIndex, startIndex + itemsPerPage);
  }, [displayedFiles, searchTerm, selectedExtensions, currentPage]);
  
  const totalPages = Math.ceil(displayedFiles.length / itemsPerPage);
  const hasMultiplePages = totalPages > 1;
  
  // Navigation functions
  const navigateToFolder = (path) => {
    setCurrentPath(path);
    setCurrentPage(1);
    setSearchTerm('');
  };
  
  const handleItemClick = (item) => {
    if (item.is_dir) {
      navigateToFolder(item.path);
    } else {
      handleFileSelect(item.path);
    }
  };

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

  // Select / deselect ALL filtered files across the whole archive
  const handleSelectAll = () => {
    if (selectedFiles.length === globalFilteredFiles.length) {
      setSelectedFiles([]);
    } else {
      setSelectedFiles(globalFilteredFiles.map(file => file.path));
    }
  }

  const handleRandomSelect = () => {
    const randomIndices = Array.from({length: Math.min(5, displayedFiles.length)}, () => Math.floor(Math.random() * displayedFiles.length));
    const randomFiles = randomIndices.map(i => displayedFiles[i]).filter(item => !item.is_dir);
    setSelectedFiles(randomFiles.map(f => f.path));
  };

// ...
  // Drag-out functionality
  const handleDragStart = (event, item) => {
    console.log('🎯 handleDragStart called for:', item.name, 'is_dir:', item.is_dir);
    
    if (item.is_dir) {
      console.log('🚫 Preventing drag for directory');
      event.preventDefault();
      return;
    }
    
    // Store file metadata for extraction on drop
    const dragData = {
      type: 'blitzarch-file',
      archivePath: archive.path || archive.archivePath || archive.file_path,
      filePath: item.path,
      fileName: item.name,
      fileSize: item.size
    };
    
    console.log('📦 Setting drag data:', dragData);
    
    try {
      event.dataTransfer.setData('application/json', JSON.stringify(dragData));
      event.dataTransfer.effectAllowed = 'copy';
      console.log('✅ Drag data set successfully');
    } catch (error) {
      console.error('❌ Error setting drag data:', error);
    }
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
        {/* Breadcrumb Navigation */}
        <div className="flex items-center gap-2 p-3 bg-slate-800/20 rounded-lg">
          {breadcrumb.map((crumb, index) => (
            <React.Fragment key={crumb.path}>
              {index > 0 && <ChevronRight className="w-4 h-4 text-slate-400" />}
              <Button
                variant="ghost"
                size="sm"
                className={`h-8 px-2 text-sm ${
                  crumb.path === currentPath 
                    ? 'bg-amber-500/20 text-amber-400' 
                    : 'text-slate-300 hover:text-white hover:bg-slate-700/50'
                }`}
                onClick={() => navigateToFolder(crumb.path)}
              >
                {index === 0 ? <Folder className="w-4 h-4" /> : crumb.name}
              </Button>
            </React.Fragment>
          ))}
        </div>
        
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-400 w-4 h-4" />
          <Input
            value={searchTerm}
            onChange={(e) => {
              setSearchTerm(e.target.value);
              setCurrentPage(1);
            }}
            placeholder="Поиск (имя или путь)..."
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
          {selectedFiles.length === globalFilteredFiles.length ? 'Снять все' : `Выбрать все (${globalFilteredFiles.length})`}
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
          onClick={() => onExtract({ archivePath: archive.path || archive.archivePath || archive.file_path, selectedFiles })}
          disabled={selectedFiles.length === 0}
          className="bg-gradient-to-r from-violet-500 to-purple-500 hover:from-violet-400 hover:to-purple-400 text-white"
        >
          <Download className="w-4 h-4 mr-2" />
          Извлечь ({selectedFiles.length})
        </Button>
      </div>

      {/* Files List */}
      <div className="max-h-96 overflow-y-auto space-y-1 bg-slate-800/20 rounded-xl p-4">
        {filteredFiles.map((item, index) => (
          <div
            key={index}
            className={`flex items-center gap-3 p-3 rounded-lg transition-all ${
              selectedFiles.includes(item.path) 
                ? 'bg-gradient-to-r from-amber-500/20 to-orange-500/20 border border-amber-500/30' 
                : 'hover:bg-slate-700/30'
            } ${!item.is_dir ? 'cursor-grab active:cursor-grabbing' : 'cursor-pointer'}`}
            onClick={(e) => {
              if (!e.defaultPrevented) {
                handleItemClick(item);
              }
            }}
            draggable={!item.is_dir}
            onDragStart={(e) => {
              console.log('🎯 Drag started for:', item.name);
              handleDragStart(e, item);
            }}
            onDrag={(e) => {
              console.log('🔄 Dragging:', item.name);
            }}
            onDragEnd={(e) => {
              console.log('🏁 Drag ended for:', item.name);
            }}
          >
            {!item.is_dir && (
              <Checkbox
                checked={selectedFiles.includes(item.path)}
                className="data-[state=checked]:bg-amber-500 data-[state=checked]:border-amber-500"
                onClick={(e) => {
                  e.stopPropagation();
                  handleFileSelect(item.path);
                }}
              />
            )}
            
            {item.is_dir ? (
              <Folder className="w-4 h-4 text-amber-400" />
            ) : (
              getFileIcon(item.name)
            )}
            
            <div className="flex-1 min-w-0">
              <p className="text-white text-sm font-medium truncate">
                {item.is_dir ? `📁 ${item.name}` : (
                  <span className="inline-flex items-center gap-1">
                    <span className="text-xs opacity-50">↗️</span>
                    {item.name}
                  </span>
                )}
              </p>
              <p className="text-slate-400 text-xs">
                {item.is_dir 
                  ? `Папка • ${formatSize(item.size)}`
                  : `${formatSize(item.size)} • CRC: ${item.crc32?.slice(0, 8) || 'N/A'}`
                }
              </p>
            </div>
            
            <div className="flex items-center gap-2">
              {item.is_dir ? (
                <ChevronRight className="w-4 h-4 text-slate-400" />
              ) : (
                item.crc_ok ? (
                  <CheckCircle className="w-4 h-4 text-emerald-400" />
                ) : (
                  <XCircle className="w-4 h-4 text-red-400" />
                )
              )}
            </div>
          </div>
        ))}
        
        {filteredFiles.length === 0 && (
          <div className="text-center py-8 text-slate-400">
            {searchTerm ? 'Файлы не найдены' : 'Папка пуста'}
          </div>
        )}
      </div>
      
      {/* Pagination */}
      {hasMultiplePages && (
        <div className="flex items-center justify-between bg-slate-800/20 rounded-lg p-3">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
            disabled={currentPage === 1}
            className="border-slate-600/50 text-slate-300 hover:bg-slate-700/40"
          >
            <ChevronLeft className="w-4 h-4 mr-1" />
            Назад
          </Button>
          
          <span className="text-slate-300 text-sm">
            Страница {currentPage} из {totalPages}
          </span>
          
          <Button
            variant="outline"
            size="sm"
            onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
            disabled={currentPage === totalPages}
            className="border-slate-600/50 text-slate-300 hover:bg-slate-700/40"
          >
            Вперед
            <ChevronRight className="w-4 h-4 ml-1" />
          </Button>
        </div>
      )}
    </div>
  );
}