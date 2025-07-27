import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { 
  TestTube, 
  CheckCircle, 
  XCircle, 
  Shield,
  Zap,
  Download,
  Upload
} from 'lucide-react';
import ResultModal from './ResultModal';

export default function TestPanel() {
  const [isVisible, setIsVisible] = useState(false);
  const [modalResult, setModalResult] = useState(null);
  const [showModal, setShowModal] = useState(false);

  const testResults = {
    create_success: {
      type: 'create_success',
      message: 'Archive created successfully with optimal compression',
      archiveName: 'documents_backup.blz',
      archiveSize: 1024 * 1024 * 45, // 45MB
      duration: 12,
      speed: 67.8,
      filesCount: 156,
      foldersCount: 8,
      compressionRatio: 72,
      compressionLevel: 9,
      outputPath: '/Users/john/Documents/BlitzArch/documents_backup.blz'
    },
    extract_success: {
      type: 'extract_success',
      message: 'All files extracted successfully with integrity verification',
      archiveName: 'project_files.blz',
      archiveSize: 1024 * 1024 * 128, // 128MB
      duration: 8,
      speed: 89.2,
      filesCount: 234,
      foldersCount: 12,
      outputPath: '/Users/john/Desktop/extracted_files/'
    },
    create_error: {
      type: 'create_error',
      message: 'Failed to create archive due to insufficient disk space',
      error: 'Error: Not enough free space on disk. Required: 2.1GB, Available: 1.8GB',
      archiveName: 'large_project.blz'
    },
    extract_error: {
      type: 'extract_error',
      message: 'Archive extraction failed due to corruption',
      error: 'Error: CRC32 checksum mismatch in file "config.json". Archive may be corrupted.',
      archiveName: 'backup_2024.blz'
    },
    password_error: {
      type: 'password_error',
      message: 'Authentication failed - incorrect password provided',
      error: 'Error: Invalid password. The archive is encrypted with AES-256.',
      archiveName: 'secure_files.blz'
    }
  };

  const showTestModal = (resultType) => {
    setModalResult(testResults[resultType]);
    setShowModal(true);
  };

  if (!isVisible) {
    return (
      <div className="fixed top-4 right-4 z-40">
        <Button
          onClick={() => setIsVisible(true)}
          size="sm"
          variant="outline"
          className="bg-neutral-800/80 border-neutral-600 text-neutral-300 hover:bg-neutral-700 backdrop-blur-sm"
        >
          <TestTube className="w-4 h-4 mr-2" />
          Test UI
        </Button>
      </div>
    );
  }

  return (
    <>
      <div className="fixed top-4 right-4 z-40">
        <Card className="w-80 bg-neutral-900/95 border-neutral-700 backdrop-blur-sm shadow-xl">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-white flex items-center gap-2 text-sm">
                <TestTube className="w-4 h-4 text-cyan-400" />
                UI Test Panel
              </CardTitle>
              <Button
                onClick={() => setIsVisible(false)}
                size="sm"
                variant="ghost"
                className="h-6 w-6 p-0 text-neutral-400 hover:text-white"
              >
                ×
              </Button>
            </div>
          </CardHeader>
          
          <CardContent className="space-y-3">
            <div>
              <div className="flex items-center gap-2 mb-2">
                <CheckCircle className="w-4 h-4 text-emerald-400" />
                <span className="text-sm font-medium text-neutral-300">Success Cases</span>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Button
                  onClick={() => showTestModal('create_success')}
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs border-emerald-600/50 text-emerald-400 hover:bg-emerald-500/10"
                >
                  <Upload className="w-3 h-3 mr-1" />
                  Create
                </Button>
                <Button
                  onClick={() => showTestModal('extract_success')}
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs border-violet-600/50 text-violet-400 hover:bg-violet-500/10"
                >
                  <Download className="w-3 h-3 mr-1" />
                  Extract
                </Button>
              </div>
            </div>

            <div>
              <div className="flex items-center gap-2 mb-2">
                <XCircle className="w-4 h-4 text-red-400" />
                <span className="text-sm font-medium text-neutral-300">Error Cases</span>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Button
                  onClick={() => showTestModal('create_error')}
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs border-red-600/50 text-red-400 hover:bg-red-500/10"
                >
                  <Upload className="w-3 h-3 mr-1" />
                  Create
                </Button>
                <Button
                  onClick={() => showTestModal('extract_error')}
                  size="sm"
                  variant="outline"
                  className="h-8 text-xs border-red-600/50 text-red-400 hover:bg-red-500/10"
                >
                  <Download className="w-3 h-3 mr-1" />
                  Extract
                </Button>
              </div>
            </div>

            <div>
              <div className="flex items-center gap-2 mb-2">
                <Shield className="w-4 h-4 text-orange-400" />
                <span className="text-sm font-medium text-neutral-300">Security</span>
              </div>
              <Button
                onClick={() => showTestModal('password_error')}
                size="sm"
                variant="outline"
                className="w-full h-8 text-xs border-orange-600/50 text-orange-400 hover:bg-orange-500/10"
              >
                <Shield className="w-3 h-3 mr-1" />
                Wrong Password
              </Button>
            </div>

            <div className="pt-2 border-t border-neutral-700">
              <div className="flex items-center justify-between text-xs text-neutral-500">
                <span>Test Environment</span>
                <Badge variant="outline" className="border-neutral-600 text-neutral-400 text-[10px]">
                  Dev Mode
                </Badge>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <ResultModal
        isOpen={showModal}
        onClose={() => setShowModal(false)}
        result={modalResult}
      />
    </>
  );
}