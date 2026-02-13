import * as vscode from 'vscode';
import { BinaryManager } from './binary-manager';
import { DependencyStatus, parseStatusOutput, getPackageName } from './types';
import * as path from 'path';
import * as fs from 'fs';

export type TreeItemType = DependencyItem | FileItem | InfoItem;

export class DependencyTreeProvider implements vscode.TreeDataProvider<TreeItemType> {
    private _onDidChangeTreeData: vscode.EventEmitter<TreeItemType | undefined | null | void> =
        new vscode.EventEmitter<TreeItemType | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<TreeItemType | undefined | null | void> =
        this._onDidChangeTreeData.event;

    private dependencies: DependencyStatus[] = [];
    private projectRoot: string;

    constructor(
        private binaryManager: BinaryManager,
        private outputChannel: vscode.OutputChannel
    ) {
        this.projectRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '';
    }

    /**
     * Refresh tree view
     */
    async refresh(): Promise<void> {
        try {
            const { stdout } = await this.binaryManager.execute(['status', '--format', 'json']);
            const statusOutput = parseStatusOutput(stdout);
            this.dependencies = statusOutput.statuses;
            this._onDidChangeTreeData.fire();
        } catch (error: any) {
            this.outputChannel.appendLine(`Failed to refresh: ${error.message}`);
            vscode.window.showErrorMessage(`Failed to refresh dependencies: ${error.message}`);
        }
    }

    /**
     * Get tree item
     */
    getTreeItem(element: TreeItemType): vscode.TreeItem {
        return element;
    }

    /**
     * Get children - ROADMAP requirement: expandable tree structure
     */
    async getChildren(element?: TreeItemType): Promise<TreeItemType[]> {
        if (!element) {
            // Root level - show all dependencies
            return this.dependencies.map(dep => new DependencyItem(dep, this.projectRoot));
        }

        // If element is a DependencyItem, show its files
        if (element instanceof DependencyItem) {
            return element.getChildren();
        }

        // Files and info items have no children
        return [];
    }

    /**
     * Set project root
     */
    setProjectRoot(rootPath: string): void {
        this.projectRoot = rootPath;
    }
}

export class DependencyItem extends vscode.TreeItem {
    constructor(
        public readonly dependency: DependencyStatus,
        private projectRoot: string
    ) {
        // ROADMAP requirement: Expandable tree structure
        super(getPackageName(dependency), vscode.TreeItemCollapsibleState.Collapsed);

        this.tooltip = this.buildTooltip();
        this.description = this.buildDescription();
        this.iconPath = this.getIcon();
        this.contextValue = 'package';

        // Remove click command - let user expand to see files instead
    }

    /**
     * Get children files for this package (ROADMAP requirement)
     */
    getChildren(): TreeItemType[] {
        const children: TreeItemType[] = [];
        const docsDir = this.getDocsDirectory();

        if (!docsDir || !this.fileExists(docsDir)) {
            return [new InfoItem('No documentation files found', 'info')];
        }

        try {
            const files = fs.readdirSync(docsDir);

            // Filter and sort files
            const docFiles = files.filter(f => {
                return f.endsWith('.md') && !f.startsWith('.aifd-meta');
            }).sort((a, b) => {
                // Priority order: _SUMMARY, README, CHANGELOG, others
                const priority: { [key: string]: number } = {
                    '_SUMMARY.md': 0,
                    'README.md': 1,
                    'CHANGELOG.md': 2,
                };
                const aPriority = priority[a] ?? 99;
                const bPriority = priority[b] ?? 99;
                return aPriority - bPriority;
            });

            // Add file items
            docFiles.forEach(file => {
                const filePath = path.join(docsDir, file);
                children.push(new FileItem(file, filePath));
            });

            // Add sync info at the end
            if (this.dependency.docs_version) {
                const syncInfo = `Synced: v${this.dependency.docs_version}`;
                children.push(new InfoItem(syncInfo, 'sync-info'));
            }

        } catch (error) {
            children.push(new InfoItem('Error reading directory', 'error'));
        }

        return children.length > 0 ? children : [new InfoItem('No files', 'info')];
    }

    private buildTooltip(): string {
        const name = getPackageName(this.dependency);
        const lines = [
            `${name}`,
            `Lock Version: ${this.dependency.lock_version}`,
            `Status: ${this.dependency.status}`,
        ];

        if (this.dependency.docs_version) {
            lines.push(`Docs Version: ${this.dependency.docs_version}`);
        }

        if (this.dependency.reason) {
            lines.push(`Reason: ${this.dependency.reason}`);
        }

        return lines.join('\n');
    }

    private buildDescription(): string {
        const version = this.dependency.lock_version;
        const status = this.dependency.status;

        if (status === 'Synced' || status === 'SyncedFallback') {
            return `${version} ✓`;
        } else if (status === 'Outdated') {
            return `${version} ⚠`;
        } else if (status === 'Missing') {
            return `${version} ✗`;
        } else {
            return `${version} 🔧`;
        }
    }

    private getIcon(): vscode.ThemeIcon {
        switch (this.dependency.status) {
            case 'Synced':
            case 'SyncedFallback':
                return new vscode.ThemeIcon('check-all', new vscode.ThemeColor('charts.green'));
            case 'Outdated':
                return new vscode.ThemeIcon('warning', new vscode.ThemeColor('charts.yellow'));
            case 'Missing':
                return new vscode.ThemeIcon('error', new vscode.ThemeColor('charts.red'));
            case 'Corrupted':
                return new vscode.ThemeIcon('tools', new vscode.ThemeColor('charts.orange'));
            default:
                return new vscode.ThemeIcon('question');
        }
    }

    private getDocsDirectory(): string | null {
        const config = vscode.workspace.getConfiguration('ai-fdocs');
        const outputDir = config.get<string>('outputDir') || 'fdocs';

        const name = getPackageName(this.dependency);
        const version = this.dependency.docs_version || this.dependency.lock_version;

        // Try Rust path first
        let docsDir = path.join(this.projectRoot, outputDir, 'rust', `${name}@${version}`);

        // Try Node.js path if Rust doesn't exist
        if (!this.fileExists(docsDir)) {
            docsDir = path.join(this.projectRoot, outputDir, 'npm', `${name}@${version}`);
        }

        return this.fileExists(docsDir) ? docsDir : null;
    }

    private fileExists(filePath: string): boolean {
        try {
            return fs.existsSync(filePath);
        } catch {
            return false;
        }
    }
}

/**
 * File item in the tree (ROADMAP requirement)
 * Represents individual documentation files like README.md, CHANGELOG.md
 */
export class FileItem extends vscode.TreeItem {
    constructor(
        public readonly fileName: string,
        public readonly filePath: string
    ) {
        super(fileName, vscode.TreeItemCollapsibleState.None);

        this.tooltip = filePath;
        this.contextValue = 'file';

        // Set icon based on file type
        this.iconPath = this.getFileIcon();

        // Click to open file
        this.command = {
            command: 'vscode.open',
            title: 'Open File',
            arguments: [vscode.Uri.file(filePath)],
        };
    }

    private getFileIcon(): vscode.ThemeIcon {
        if (this.fileName === '_SUMMARY.md') {
            return new vscode.ThemeIcon('book');
        } else if (this.fileName === 'README.md') {
            return new vscode.ThemeIcon('file-text');
        } else if (this.fileName === 'CHANGELOG.md') {
            return new vscode.ThemeIcon('list-ordered');
        } else {
            return new vscode.ThemeIcon('file');
        }
    }
}

/**
 * Info item in the tree (ROADMAP requirement)
 * Shows metadata like "Synced 2 days ago"
 */
export class InfoItem extends vscode.TreeItem {
    constructor(
        public readonly text: string,
        public readonly infoType: 'sync-info' | 'error' | 'info'
    ) {
        super(text, vscode.TreeItemCollapsibleState.None);

        this.contextValue = 'info';

        // Set icon based on info type
        if (infoType === 'sync-info') {
            this.iconPath = new vscode.ThemeIcon('info', new vscode.ThemeColor('charts.blue'));
        } else if (infoType === 'error') {
            this.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('charts.red'));
        } else {
            this.iconPath = new vscode.ThemeIcon('info');
        }
    }
}
