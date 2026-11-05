// SPDX-License-Identifier: Apache-2.0
import type { Module, OperationDiagnostic, PartitionDiagnostic } from "./types";

const objects = (value: unknown): Record<string, unknown>[] =>
  Array.isArray(value)
    ? value.filter(
        (row): row is Record<string, unknown> => !!row && typeof row === "object",
      )
    : [];
const text = (value: unknown): string => (typeof value === "string" ? value : "");

export function normalizePartitionErrors(value: unknown): PartitionDiagnostic[] {
  return objects(value)
    .map((row) => ({
      partition: text(row.partition),
      code: text(row.code),
      reason: text(row.reason),
      modules: Array.isArray(row.modules)
        ? row.modules.filter((id): id is string => typeof id === "string")
        : [],
    }))
    .filter((row) => row.partition.length > 0 && row.reason.length > 0);
}

export function normalizeOperationErrors(value: unknown): OperationDiagnostic[] {
  return objects(value)
    .map((row) => ({
      module_id: text(row.module_id),
      logical_path: text(row.logical_path),
      target: typeof row.target === "string" ? row.target : null,
      code: text(row.code),
      reason: text(row.reason),
    }))
    .filter((row) => row.module_id.length > 0 && row.reason.length > 0);
}

export function moduleDiagnosticSummary(module: Module): string {
  return [
    ...(module.partition_errors ?? []).map((e) => `${e.partition}: ${e.reason}`),
    ...(module.operation_errors ?? []).map((e) => `${e.logical_path}: ${e.reason}`),
  ].join("\n");
}
