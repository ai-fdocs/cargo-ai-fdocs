import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export type ProjectType = 'rust' | 'nodejs' | 'unknown';

export interface ProjectInfo {
    type: ProjectType;
    rootPath: string;
    hasConfig: boolean; // ai-fdocs.toml exists
    lockfilePath?: string;
    configPath?: string;
}

export class ProjectDetector {
    private watchers: vscode.FileSystemWatcher[] = [];

    /**
     * Detect all projects in workspace
     */
    async detectProjects(): Promise<ProjectInfo[]> {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders) {
            return [];
        }

        const projects: ProjectInfo[] = [];

        for (const folder of workspaceFolders) {
            const project = await this.detectProjectInFolder(folder.uri.fsPath);
            if (project) {
                projects.push(project);
            }
        }

        return projects;
    }

    /**
     * Detect project type in a specific folder
     */
    private async detectProjectInFolder(folderPath: string): Promise<ProjectInfo | null> {
        const cargoToml = path.join(folderPath, 'Cargo.toml');
        const cargoLock = path.join(folderPath, 'Cargo.lock');
        const packageJson = path.join(folderPath, 'package.json');
        const packageLock = path.join(folderPath, 'package-lock.json');
        const aifdocsToml = path.join(folderPath, 'ai-fdocs.toml');

        // Check for Rust project
        if (this.fileExists(cargoToml)) {
            return {
                type: 'rust',
                rootPath: folderPath,
                hasConfig: this.fileExists(aifdocsToml),
                lockfilePath: this.fileExists(cargoLock) ? cargoLock : undefined,
                configPath: this.fileExists(aifdocsToml) ? aifdocsToml : undefined,
            };
        }

        // Check for Node.js project
        if (this.fileExists(packageJson)) {
            return {
                type: 'nodejs',
                rootPath: folderPath,
                hasConfig: this.fileExists(aifdocsToml),
                lockfilePath: this.fileExists(packageLock) ? packageLock : undefined,
                configPath: this.fileExists(aifdocsToml) ? aifdocsToml : undefined,
            };
        }

        // Check if only ai-fdocs.toml exists
        if (this.fileExists(aifdocsToml)) {
            return {
                type: 'unknown',
                rootPath: folderPath,
                hasConfig: true,
                configPath: aifdocsToml,
            };
        }

        return null;
    }

    /**
     * Check if file exists
     */
    private fileExists(filePath: string): boolean {
        try {
            return fs.existsSync(filePath);
        } catch {
            return false;
        }
    }

    /**
     * Setup file watchers for project changes
     */
    setupWatchers(callback: () => void): void {
        // Dispose existing watchers
        this.disposeWatchers();

        // Watch for lockfile changes
        const cargoLockWatcher = vscode.workspace.createFileSystemWatcher('**/Cargo.lock');
        const packageLockWatcher = vscode.workspace.createFileSystemWatcher('**/package-lock.json');
        const configWatcher = vscode.workspace.createFileSystemWatcher('**/ai-fdocs.toml');

        cargoLockWatcher.onDidChange(callback);
        cargoLockWatcher.onDidCreate(callback);
        cargoLockWatcher.onDidDelete(callback);

        packageLockWatcher.onDidChange(callback);
        packageLockWatcher.onDidCreate(callback);
        packageLockWatcher.onDidDelete(callback);

        configWatcher.onDidChange(callback);
        configWatcher.onDidCreate(callback);
        configWatcher.onDidDelete(callback);

        this.watchers.push(cargoLockWatcher, packageLockWatcher, configWatcher);
    }

    /**
     * Dispose all watchers
     */
    disposeWatchers(): void {
        this.watchers.forEach(watcher => watcher.dispose());
        this.watchers = [];
    }

    /**
     * Get project type emoji
     */
    static getProjectEmoji(type: ProjectType): string {
        switch (type) {
            case 'rust':
                return '🦀';
            case 'nodejs':
                return '📦';
            default:
                return '📚';
        }
    }

    /**
     * Get project type display name
     */
    static getProjectTypeName(type: ProjectType): string {
        switch (type) {
            case 'rust':
                return 'Rust';
            case 'nodejs':
                return 'Node.js';
            default:
                return 'Unknown';
        }
    }
}
