-- Seed data for testing: a campaign with a source and chat message
CREATE campaign SET
  id = 'test-campaign',
  name = 'Test Campaign',
  system = 'D&D 5e',
  created_at = time::now(),
  updated_at = time::now();

CREATE setting:`llm_provider` SET value = 'openai';
CREATE setting:`llm_model` SET value = 'gpt-4o-mini';
CREATE message SET
  id = 'msg-001',
  campaign = campaign:test-campaign,
  role = 'user',
  content = 'What is a shortsword?',
  created_at = time::now();
