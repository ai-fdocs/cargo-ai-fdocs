export interface StatusSummary {
    total: number;
    synced: number;
    missing: number;
    outdated: number;
    corrupted: number;
}

export interface DependencyStatus {
    crate_name?: string; // For Rust
    package_name?: string; // For Node.js
    lock_version: string;
    docs_version?: string;
    status: 'Synced' | 'SyncedFallback' | 'Outdated' | 'Missing' | 'Corrupted';
    reason?: string;
    mode?: string;
    source_kind?: string;
    reason_code?: string;
}

export interface StatusOutput {
    summary: StatusSummary;
    statuses: DependencyStatus[];
}

export function parseStatusOutput(jsonOutput: string): StatusOutput {
    try {
        const parsed = JSON.parse(jsonOutput);
        return parsed as StatusOutput;
    } catch (error) {
        throw new Error(`Failed to parse status output: ${error}`);
    }
}

export function getPackageName(dep: DependencyStatus): string {
    return dep.crate_name || dep.package_name || 'unknown';
}
