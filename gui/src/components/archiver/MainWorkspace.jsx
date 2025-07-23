
import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { 
  Upload, 
  Folder, 
  Download,
  FileArchive,
  Layers,
  Zap
} from 'lucide-react';

import WorkspaceDropZone from './WorkspaceDropZone';
import ArchiveExplorer from './ArchiveExplorer';
import ProcessingInterface from './ProcessingInterface';

const MODES = {
  create: {
    title: 'Archive Creation',
    subtitle: 'Compress files efficiently',
    icon: FileArchive,
    color: 'teal',
    gradient: 'from-teal-500 to-cyan-500'
  },
  browse: {
    title: 'Archive Explorer',
    subtitle: 'Browse archive contents',
    icon: Layers,
    color: 'emerald',
    gradient: 'from-emerald-500 to-teal-500'
  },
  extract: {
    title: 'Batch Extraction',
    subtitle: 'Extract multiple archives',
    icon: Zap,
    color: 'cyan',
    gradient: 'from-cyan-500 to-blue-500'
  }
};

export default function MainWorkspace({ 
  activeMode,
  setActiveMode,
  onCreateArchive,
  onLoadArchive,
  onExtractArchive,
  selectedArchive,
  setSelectedArchive,
  isProcessing,
  processingType,
  progress,
  speed
}) {

  return (
    <div className="space-y-6">
      <Card className="bg-neutral-800/40 border-neutral-700">
        <CardContent className="p-6 md:p-8">
          
          {/* Mode Selector */}
          <div className="mb-8 grid grid-cols-1 sm:grid-cols-3 gap-3">
            {Object.entries(MODES).map(([key, mode]) => {
              const Icon = mode.icon;
              const isActive = activeMode === key;
              
              return (
                <Button
                  key={key}
                  variant={isActive ? "default" : "outline"}
                  onClick={() => setActiveMode(key)}
                  disabled={isProcessing}
                  className={`h-16 px-4 text-left flex items-center gap-3 transition-all duration-300 ${
                    isActive 
                      ? `bg-gradient-to-r ${mode.gradient} text-white shadow-lg shadow-${mode.color}-500/25 border-0` 
                      : 'bg-neutral-800/30 border-neutral-600 text-neutral-300 hover:bg-neutral-700/50 hover:border-neutral-500'
                  }`}
                >
                  <Icon className="w-5 h-5 flex-shrink-0" />
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium truncate">{mode.title}</div>
                    <div className={`text-xs opacity-80 truncate ${isActive ? 'text-white/80' : 'text-neutral-400'}`}>
                      {mode.subtitle}
                    </div>
                  </div>
                </Button>
              );
            })}
          </div>

          {/* Processing Interface Overlay */}
          <div className="relative min-h-[450px]">
            <AnimatePresence>
              {isProcessing && (
                <ProcessingInterface 
                  progress={progress}
                  speed={speed}
                  type={processingType}
                />
              )}
            </AnimatePresence>

            {/* Mode Content */}
            <AnimatePresence mode="wait">
              {!isProcessing && (
                <motion.div
                  key={activeMode}
                  initial={{ opacity: 0, y: 20 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -20 }}
                  transition={{ duration: 0.3 }}
                >
                  {activeMode === 'create' && (
                    <WorkspaceDropZone 
                      onFilesSelected={onCreateArchive}
                      title="Drop Files to Create Archive"
                      subtitle="Drag & drop any files or folders here to begin"
                      acceptMultiple={true}
                      color="teal"
                    />
                  )}

                  {activeMode === 'browse' && (
                    <>
                      {!selectedArchive ? (
                        <WorkspaceDropZone 
                          onFilesSelected={(files) => onLoadArchive(files[0])}
                          title="Load Archive for Browsing"
                          subtitle="Drop a .blz archive to explore its contents"
                          acceptArchives={true}
                          color="emerald"
                        />
                      ) : (
                        <ArchiveExplorer 
                          archive={selectedArchive}
                          onExtract={onExtractArchive}
                          onClose={() => setSelectedArchive(null)}
                        />
                      )}
                    </>
                  )}

                  {activeMode === 'extract' && (
                    <WorkspaceDropZone 
                      onFilesSelected={(files) => {
                        setTimeout(() => onExtractArchive(files), 800);
                      }}
                      title="Batch Extraction Mode"
                      subtitle="Drop multiple archives to extract all contents"
                      acceptArchives={true}
                      acceptMultiple={true}
                      color="cyan"
                    />
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
