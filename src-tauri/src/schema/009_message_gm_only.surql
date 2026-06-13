-- 009_message_gm_only.surql
-- Phase 2: flag assistant messages whose answer drew on GM-secret material so
-- the chat UI can show a shield indicator. Set when any retrieved chunk used to
-- build the answer is is_gm_only. Defaults false (existing messages stay clean).

DEFINE FIELD is_gm_only ON TABLE message TYPE bool DEFAULT false;
