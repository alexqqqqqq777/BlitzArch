import React, { useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
  Filter,
  FolderOpen,
  Folder,
  ChevronRight,
  ChevronLeft,
  Home
} from 'lucide-react';

export default function ArchiveExplorer({ archive, onExtract, onClose }) {
  const isMac = typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedFiles, setSelectedFiles] = useState([]);
  const [selectedExtensions, setSelectedExtensions] = useState(['all']);
  const [currentPath, setCurrentPath] = useState('/');
  const [currentPage, setCurrentPage] = useState(1);
  const itemsPerPage = 50;

  // Build folder hierarchy and current level items
  const { displayedFiles, totalItems, breadcrumb, extensions } = useMemo(() => {
    if (!archive?.files || archive.files.length === 0) {
      return { displayedFiles: [], totalItems: 0, breadcrumb: [], extensions: ['all'] };
    }
    
    // Build folder structure from file paths
    const folderStructure = new Map();
    
    let sanitizeCount = 0;
    archive.files.forEach((file, index) => {
      // Sanitize potential recursive objects
      if (typeof file.path === 'object' && file.path !== null) {
        sanitizeCount++; // silent count
        file.path = file.path.path || '';
      }
      if (typeof file.name === 'object' && file.name !== null) {
        sanitizeCount++; // silent count
        file.name = file.name.name || '';
      }

      // Safe path validation
      if (!file || !file.path || typeof file.path !== 'string') {
        console.warn(`⚠️ Invalid file at index ${index}:`, file);
        return; // Skip this file
      }
      
      const pathParts = file.path.split('/').filter(p => p);
      let currentLevel = folderStructure;
      
      // Build nested folder structure
      for (let i = 0; i < pathParts.length - 1; i++) {
        const folderName = pathParts[i];
        
        if (!currentLevel.has(folderName)) {
          currentLevel.set(folderName, { folders: new Map(), files: [] });
        }
        currentLevel = currentLevel.get(folderName).folders;
      }
      
      // Add file to its parent folder
      if (pathParts.length > 1) {
        const parentPath = pathParts.slice(0, -1);
        let targetLevel = folderStructure;
        for (let i = 0; i < parentPath.length - 1; i++) {
          if (targetLevel.has(parentPath[i])) {
            targetLevel = targetLevel.get(parentPath[i]).folders;
          }
        }
        if (targetLevel.has(parentPath[parentPath.length - 1])) {
          targetLevel.get(parentPath[parentPath.length - 1]).files.push(file);
        }
      } else {
        // Root level file
        if (!folderStructure.has('__root_files__')) {
          folderStructure.set('__root_files__', { folders: new Map(), files: [] });
        }
        folderStructure.get('__root_files__').files.push(file);
      } // end else root file
    }); // end forEach

    if (sanitizeCount > 0) {
      console.info(`🔧 Sanitized ${sanitizeCount} recursive object fields in ArchiveExplorer`);
    }
    
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
      breadcrumb,
      extensions 
    };
  }, [archive?.files, currentPath]);

  const filteredFiles = useMemo(() => {
    let filtered = displayedFiles.filter(item => {
      const matchesSearch = item.name.toLowerCase().includes(searchTerm.toLowerCase());
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

  // Drag-out functionality for external drop
  const handleDragStart = async (event, item) => {
    console.log('🎯 handleDragStart called for:', item.name, 'is_dir:', item.is_dir);

    if (item.is_dir) {
      console.log('🚫 Preventing drag for directory');
      event.preventDefault();
      return;
    }
    window.__ba_drag_active = true;

    if (isMac) {
      // Попытка нативного drag-out через плагин
      console.log('🧲 Invoking native_drag_out…');
      try {
        await invoke('plugin:dragout|native_drag_out', {
          archivePath: archive.path || archive.archivePath || archive.file_path,
          archive_path: archive.path || archive.archivePath || archive.file_path,
          filePaths: [item.path],
          file_paths: [item.path],
          targetDir: null,
          target_dir: null
        });
        // Успешно запустили нативный drag – гасим HTML5-сессию
        if (event.preventDefault) event.preventDefault();
        if (event.stopPropagation) event.stopPropagation();
        if (event.dataTransfer) {
          try { event.dataTransfer.clearData(); } catch (_) {}
        }
        setTimeout(() => { window.__ba_drag_active = false; }, 3000);
        return;
      } catch (e) {
        console.warn('⚠️ native_drag_out plugin call failed, trying global command:', e);
        try {
          await invoke('native_drag_out_global', {
            // camelCase и snake_case для полной совместимости
            archivePath: archive.path || archive.archivePath || archive.file_path,
            archive_path: archive.path || archive.archivePath || archive.file_path,
            filePaths: [item.path],
            file_paths: [item.path],
            targetDir: null,
            target_dir: null
          });
          return; // успех через глобальную команду
        } catch (e2) {
          console.warn('⚠️ native_drag_out_global also failed, falling back to extraction:', e2);
// -- лишний дублирующий блок удалён --
          }
          // продолжаем на fallback ниже
        }
      }

      // ---------- Non-macOS fallback: instant extraction then link ----------
      console.log('🚀 Starting instant extraction fallback for drag-out…');
      try {
    
      // Get Downloads directory
      const downloadsDir = await invoke('get_downloads_path').catch(() => '/Users/oleksandr/Downloads');
      const tempDir = `${downloadsDir}/BlitzArch_DragOut`;
      
      // Extract file instantly to Downloads for drag-out
      console.log('📦 Extracting file for drag-out:', item.name);
      const result = await invoke('drag_out_extract', {
        // Передаем camelCase и snake_case версии для полной совместимости
        archivePath: archive.path || archive.archivePath || archive.file_path,
        archive_path: archive.path || archive.archivePath || archive.file_path,
        filePath: item.path,
        file_path: item.path,
        targetDir: tempDir,
        target_dir: tempDir,
        password: null // TODO: get from settings if needed
      });
      
      if (result.success && result.archive_path) {
        console.log('✅ File extracted for drag-out:', result.archive_path);

        // Create link file (.webloc / .url) via fs-extra plugin
        // use backend command to create link file
        const isMac = navigator.platform.toLowerCase().includes('mac');
        const linkExt = isMac ? 'webloc' : 'url';
        const linkName = `${item.name}.${linkExt}`;
        const linkPath = `${tempDir}/${linkName}`;
        const fileUri = `file://${result.archive_path}`;
        let linkContent = '';
        const fileUriEsc = fileUri.replace(/ /g, '%20');
        if (isMac) {
          linkContent = `<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>URL</key>\n  <string>${fileUriEsc}</string>\n</dict>\n</plist>`;
        } else {
          linkContent = `[InternetShortcut]\nURL=${fileUriEsc}\n`;
        }
        try {
          await invoke('create_link_file', { path: linkPath, contents: linkContent });
          console.log('🔗 Link file created:', linkPath);
          // Copy original file path to clipboard as convenience
          if (navigator.clipboard) {
            navigator.clipboard.writeText(result.archive_path).catch(() => {});
          }

          // Put link file path to dataTransfer
          event.dataTransfer.setData('text/plain', linkPath);
          event.dataTransfer.setData('text/uri-list', `file://${linkPath}\n`);
          event.dataTransfer.setData('DownloadURL', `application/octet-stream:${linkName}:file://${linkPath}`);
        } catch (e) {
          console.error('❌ Failed to create link file:', e);
        }
        
        // Set the REAL file path for system drag-out
        event.dataTransfer.setData('text/plain', result.archive_path);
        // Provide multiple MIME types for max OS compatibility
          const downloadUrl = `application/octet-stream:${item.name}:${fileUri}`;
          event.dataTransfer.setData('DownloadURL', downloadUrl);
          // macOS Finder
          event.dataTransfer.setData('public.file-url', fileUri);
          event.dataTransfer.setData('public.url', fileUri);
          event.dataTransfer.setData('public.url-name', item.name);
          // Windows / GTK variations
          event.dataTransfer.setData('text/x-moz-url', fileUri);
          event.dataTransfer.setData('text/x-moz-url-data', fileUri);
          event.dataTransfer.setData('text/x-moz-url-desc', item.name);
          event.dataTransfer.setData('text/uri-list', `${fileUri}\n`);
          event.dataTransfer.setData('text/html', `<a href="${fileUri}">${item.name}</a>`);
          event.dataTransfer.setData('URL', fileUri);
        event.dataTransfer.effectAllowed = 'copy';
        
        console.log('✅ Real file path set for drag-out');
      } else {
        console.error('❌ Failed to extract file:', result.error);
        event.preventDefault();
      }
      } catch (fallbackErr) {
        console.error('❌ Error in instant extraction fallback:', fallbackErr);
        event.preventDefault();
      }

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

      {/* Breadcrumb Navigation */}
      <div className="flex items-center gap-2 p-3 bg-neutral-800/20 rounded-lg border border-neutral-700">
        {breadcrumb.map((crumb, index) => (
          <React.Fragment key={crumb.path}>
            {index > 0 && <ChevronRight className="w-4 h-4 text-neutral-400" />}
            <Button
              variant="ghost"
              size="sm"
              className={`h-8 px-2 text-sm ${
                crumb.path === currentPath 
                  ? 'bg-teal-500/20 text-teal-400 hover:bg-teal-500/30' 
                  : 'text-neutral-300 hover:text-white hover:bg-neutral-700'
              }`}
              onClick={() => navigateToFolder(crumb.path)}
            >
              {index === 0 ? <Home className="w-4 h-4" /> : crumb.name}
            </Button>
          </React.Fragment>
        ))}
      </div>

      {/* Search and Filters */}
      <div className="space-y-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-neutral-400 w-4 h-4" />
          <Input
            value={searchTerm}
            onChange={(e) => {
              setSearchTerm(e.target.value);
              setCurrentPage(1);
            }}
            placeholder="Search in current folder..."
            className="pl-10 bg-neutral-800/30 border-neutral-600 text-white placeholder-neutral-500"
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
          size="sm"
          onClick={handleSelectAll}
          className="bg-gradient-to-r from-teal-500 to-cyan-500 hover:from-teal-400 hover:to-cyan-400 text-white border-0"
        >
          {selectedFiles.length === filteredFiles.length ? 'Deselect All' : 'Select All'}
        </Button>
        
        <Button
          size="sm"
          onClick={() => onExtract({ 
            archivePath: archive.path || archive.file_path,
            selectedFiles: selectedFiles.length > 0 ? selectedFiles.map(p => p.startsWith('/') ? p.slice(1) : p) : null 
          })}
          disabled={false} // Allow extracting whole archive if no selection
          className="bg-gradient-to-r from-teal-500 to-cyan-500 hover:from-teal-400 hover:to-cyan-400 text-white border-0"
        >
          <Download className="w-4 h-4 mr-2" />
          {selectedFiles.length > 0 
            ? `Extract Selected (${selectedFiles.length})` 
            : 'Extract All Files'
          }
        </Button>
      </div>

      {/* Files List */}
      <div className="bg-neutral-800/30 rounded-xl border border-neutral-700">
        <div className="max-h-80 overflow-y-auto">
          <div className="p-4 space-y-1">
            {filteredFiles.map((item, index) => (
              <div
                key={index}
                className={`flex items-center gap-3 p-3 rounded-lg transition-all ${
                  selectedFiles.includes(item.path) 
                    ? 'bg-gradient-to-r from-teal-500/20 to-cyan-500/20 border border-teal-500/30' 
                    : 'hover:bg-neutral-700/30'
                } ${!item.is_dir ? 'cursor-grab active:cursor-grabbing' : 'cursor-pointer'}`}
                onClick={(e) => {
                  if (!e.defaultPrevented) {
                    handleItemClick(item);
                  }
                }}
                draggable={!item.is_dir && !isMac}
                onMouseDown={(e) => {
              if (isMac && !item.is_dir) {
                handleDragStart(e, item);
                return; // skip default
              }
            }}
            onDragStart={(e) => {
                  console.log('🎯 Drag started for:', item.name);
                  if (!isMac) {
                handleDragStart(e, item);
              }
                }}
                onDrag={(e) => {
                  console.log('🔄 Dragging:', item.name);
                }}
                onDragEnd={(e) => {
                  console.log('🏁 Drag ended for:', item.name);
                }}
              >
                <Checkbox
                  checked={selectedFiles.includes(item.path)}
                  className="data-[state=checked]:bg-teal-500 data-[state=checked]:border-teal-500"
                  onMouseDown={(e) => e.stopPropagation()}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleFileSelect(item.path);
                  }}
                />
                
                {item.is_dir ? (
                  <Folder className={selectedFiles.includes(item.path) ? "w-4 h-4 text-amber-400" : "w-4 h-4 text-teal-400"} />
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
                  <div className="flex items-center gap-4 text-xs text-neutral-400">
                    <span>{formatSize(item.size)}</span>
                    {!item.is_dir && (
                      <>
                        <span>CRC: {item.crc32?.slice(0, 8) || 'N/A'}</span>
                        <span className="text-neutral-500">•</span>
                        <span className="truncate">{item.path}</span>
                      </>
                    )}
                  </div>
                </div>
                
                <div className="flex items-center gap-2">
                  {item.is_dir ? (
                    <ChevronRight className="w-4 h-4 text-neutral-400" />
                  ) : (
                    item.crc_ok ? (
                      <div className="flex items-center gap-1">
                        <CheckCircle className="w-4 h-4 text-emerald-400" />
                        <span className="text-xs text-emerald-400">Valid</span>
                      </div>
                    ) : (
                      <div className="flex items-center gap-1">
                        <XCircle className="w-4 h-4 text-red-400" />
                        <span className="text-xs text-red-400">Error</span>
                      </div>
                    )
                  )}
                </div>
              </div>
            ))}
            
            {filteredFiles.length === 0 && (
              <div className="text-center py-12 text-neutral-500">
                <File className="w-12 h-12 mx-auto mb-3 opacity-50" />
                <p>{searchTerm ? 'No files match your search' : 'This folder is empty'}</p>
              </div>
            )}
          </div>
        </div>
        
        {/* Pagination */}
        {hasMultiplePages && (
          <div className="flex items-center justify-between p-3 border-t border-neutral-700">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
              disabled={currentPage === 1}
              className="border-neutral-600 text-neutral-300 hover:bg-neutral-700"
            >
              <ChevronLeft className="w-4 h-4 mr-1" />
              Previous
            </Button>
            
            <span className="text-neutral-300 text-sm">
              Page {currentPage} of {totalPages} • {totalItems} items
            </span>
            
            <Button
              variant="outline"
              size="sm"
              onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
              disabled={currentPage === totalPages}
              className="border-neutral-600 text-neutral-300 hover:bg-neutral-700"
            >
              Next
              <ChevronRight className="w-4 h-4 ml-1" />
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}