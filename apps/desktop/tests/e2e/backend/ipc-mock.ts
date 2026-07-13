import type { Page } from '@playwright/test';

/**
 * Install the Tauri IPC mock into the page before app scripts run.
 *
 * We mock window.__TAURI_INTERNALS__.invoke directly instead of importing
 * mockIPC from @tauri-apps/api/mocks, because addInitScript runs in the
 * browser context where the module isn't available.
 *
 * Every invoke() is recorded into window.__ipcCalls so tests and BDD steps
 * can assert which commands were (not) sent.
 *
 * @param overrides - Optional map of command names to response values to override defaults
 */
export async function installIpcMock(
  page: Page,
  overrides?: Record<string, unknown>,
): Promise<void> {
  await page.addInitScript(
    ({ overridesArg }) => {
      let _cbId = 0;
      // @ts-expect-error -- injected by Tauri at runtime
      // eslint-disable-next-line @typescript-eslint/no-empty-function
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      // @ts-expect-error -- test-only call log
      window.__ipcCalls = [] as Array<{ cmd: string; args?: Record<string, unknown> }>;
      // @ts-expect-error -- injected by Tauri at runtime
      window.__TAURI_INTERNALS__ = {
        transformCallback: (_cb: unknown, _once?: boolean) => ++_cbId,
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          // @ts-expect-error -- test-only call log
          window.__ipcCalls.push({ cmd, args });
          // Check overrides first
          if (overridesArg && cmd in overridesArg) {
            return Promise.resolve(overridesArg[cmd as keyof typeof overridesArg]);
          }
          switch (cmd) {
          case 'plugin:event|listen':
            return Promise.resolve(0);
          case 'plugin:event|unlisten':
            return Promise.resolve(null);
          case 'plugin:os|locale':
            return Promise.resolve('en-US');
          case 'get_embedding_provider_status':
            return Promise.resolve({
              backend: 'openai',
              model: 'text-embedding-3-small',
              dimension: 1536,
              api_key_configured: true,
              local_available: false,
              local_cached: false,
            });
          case 'get_embedding_model_mismatch':
            return Promise.resolve({ active_model: 'nomic-embed-text-v1.5', stale: [] });
          case 'get_campaigns':
            return Promise.resolve([{ id: 'camp1', name: 'Test Campaign', system: 'D&D 5e' }]);
          case 'get_entity_counts':
            return Promise.resolve({});
          case 'get_sessions':
            return Promise.resolve([]);
          case 'get_collections':
            return Promise.resolve([]);
          case 'get_sources':
            return Promise.resolve([]);
          case 'get_settings':
            return Promise.resolve({
              llm_provider: 'openai',
              llm_model: 'gpt-4o-mini',
              llm_api_key: 'sk-test',
              llm_base_url: '',
              active_campaign_id: '',
            });
          case 'update_setting':
            return Promise.resolve(null);
          case 'get_llm_provider_status':
            return Promise.resolve({
              provider_type: 'openai',
              model: 'gpt-4o-mini',
              api_key_configured: true,
            });
          case 'get_chat_history':
            return Promise.resolve([]);
          case 'chat_send':
            return Promise.resolve(null);
          case 'get_custom_providers':
            return Promise.resolve([]);
          case 'get_provider_models':
            return Promise.resolve([]);
          case 'delete_campaign':
            return Promise.resolve(null);
          case 'get_campaign_collections':
            return Promise.resolve([]);
          case 'get_codex_status':
            return Promise.resolve({
              stale_entities: 12,
              total_entities: 40,
              rules_stale: 0,
              rule_entries: 0,
            });
          case 'compile_collection':
            return Promise.resolve({ articles_compiled: 12, remaining_stale: 0 });
          case 'get_rule_entries':
            return Promise.resolve([
              {
                id: 'rule1',
                name: 'Initiative',
                category: 'mechanic',
                body: 'Roll a d20 and add your Dexterity modifier to determine turn order.',
                notes: null,
                page_refs: [{ source_name: 'Core Rulebook', page_start: 12, page_end: 13 }],
                stale: false,
              },
            ]);
          case 'update_rule_notes':
            return Promise.resolve(null);
          case 'redo_rule_entry':
            return Promise.resolve(null);
          case 'save_chat_to_codex':
            return Promise.resolve(0);
          case 'get_proposals':
            return Promise.resolve([]);
          case 'get_maintenance_counts':
            return Promise.resolve({ pending_proposals: 0, unresolved_findings: 0 });
          case 'accept_proposal':
            return Promise.resolve(null);
          case 'reject_proposal':
            return Promise.resolve(null);
          case 'get_vault_path':
            return Promise.resolve(null);
          case 'set_vault_path':
            return Promise.resolve(null);
          case 'vault_sync_now':
            return Promise.resolve({
              exported: 0,
              unchanged: 0,
              adopted: 0,
              applied: 0,
              conflicts: 0,
              resolved: 0,
              soft_deleted: 0,
              swept: 0,
              invalid: 0,
              failed: 0,
            });
          case 'list_vault_conflicts':
            return Promise.resolve([]);
          case 'soft_delete_entity':
            return Promise.resolve(null);
          default:
            console.warn(`Unhandled IPC mock: ${cmd}`);
            return Promise.resolve(null);
          }
        },
      };
    },
    { overridesArg: overrides },
  );
}
