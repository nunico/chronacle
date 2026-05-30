/** Sample chat history for testing the chat history display */
export const mockChatHistory = [
  { role: 'user', content: 'What is a shortsword?' },
  { role: 'assistant', content: 'A shortsword deals 1d6 piercing damage. [Source: "Core Rulebook", p.145]' },
];

/** Sample custom provider for testing provider configuration */
export const mockCustomProvider = {
  id: 'prov-001',
  name: 'OpenRouter',
  provider_type: 'openai',
  base_url: 'https://openrouter.ai/api/v1',
  api_key: 'sk-or-test',
};

/** Sample models for a custom provider */
export const mockProviderModels = [
  { id: 'mod-001', provider_id: 'prov-001', model_id: 'gpt-4o', display_name: 'GPT-4o' },
  { id: 'mod-002', provider_id: 'prov-001', model_id: 'claude-3-haiku', display_name: 'Claude 3 Haiku' },
];
