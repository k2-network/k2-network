# Tauri IPC Bridge Integration Guide

This document describes how to integrate k2-core's agent loop with a Tauri desktop application.

## Overview

The k2-core agent loop can be controlled from a Tauri frontend through IPC commands. This enables chat-with-agent functionality in the Tauri UI.

## IPC Commands

### 1. Initialize Agent Session

```rust
#[tauri::command]
async fn agent_init(
    session_id: String,
    llm_provider: String,
    model: String,
    store: tauri::State<'_, SqliteStore>,
) -> Result<String, String> {
    let executor = AgentLoopExecutor::builder()
        .session_id(session_id.clone())
        .model(model)
        .store(store.inner().clone())
        .build()
        .map_err(|e| e.to_string())?;
    
    // Store executor in app state for later use
    Ok(session_id)
}
```

### 2. Send User Message

```rust
#[tauri::command]
async fn agent_chat(
    session_id: String,
    message: String,
    app_state: tauri::State<'_, AppState>,
) -> Result<AgentResponse, String> {
    let mut executor = app_state.get_executor(&session_id)
        .ok_or("Session not found")?;
    
    let input = UserInput {
        role: MessageRole::User,
        content: message,
    };
    
    let outcome = executor.run(input)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(AgentResponse::from(outcome))
}
```

### 3. Handle Approval Requests

```rust
#[tauri::command]
async fn agent_approve(
    request_id: String,
    allowed: bool,
    resolver: tauri::State<'_, ApprovalResolver>,
) -> Result<(), String> {
    if allowed {
        let approval = LeaseApproval::new(
            "tauri-ui".to_string(),
            vec![EffectKind::All],
            None,
            None,
            None,
        );
        resolver.approve_dispatch(&request_id, approval).await
            .map_err(|e| e.to_string())?;
    } else {
        resolver.deny(&request_id, "User denied".to_string()).await
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
```

### 4. List Pending Approvals

```rust
#[tauri::command]
async fn list_pending_approvals(
    store: tauri::State<'_, SqliteStore>,
) -> Result<Vec<ApprovalRequest>, String> {
    let approvals = store.list_pending_approvals()
        .map_err(|e| e.to_string())?;
    
    Ok(approvals.into_iter().map(|r| ApprovalRequest::from(r)).collect())
}
```

## Frontend Integration

### TypeScript

```typescript
import { invoke } from '@tauri-apps/api/tauri'

// Initialize agent
async function initAgent(provider: string, model: string) {
  const sessionId = await invoke('agent_init', {
    llmProvider: provider,
    model
  })
  return sessionId
}

// Send message
async function sendMessage(sessionId: string, message: string) {
  const response = await invoke('agent_chat', {
    sessionId,
    message
  })
  return response
}

// Approve a request
async function approveRequest(requestId: string, allowed: boolean) {
  await invoke('agent_approve', {
    requestId,
    allowed
  })
}

// Poll for pending approvals
async function getPendingApprovals() {
  const approvals = await invoke('list_pending_approvals')
  return approvals
}
```

## Tool Calls Handling

When the agent requires tool execution, the Tauri UI should:

1. Poll `list_pending_approvals` periodically
2. Display approval dialog to user
3. Call `agent_approve` with user's decision
4. Await final response from `agent_chat`

## Error Handling

All IPC commands return `Result<T, String>`. Errors include:
- Session not found
- LLM provider errors
- Checkpoint load failures
- Approval resolution errors
- Tool invocation errors

## Security Considerations

- **Tool Trust**: Only UserTrusted and higher tools require approval
- **Fail-closed**: Deny all requests by default, require explicit approval
- **Checkpoint Recovery**: Agent state persists across app restarts
- **LocalUser**: Tauri UI is treated as LocalUser (highest trust)

## Example React Component

```typescript
function AgentChat() {
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [pendingApprovals, setPendingApprovals] = useState<ApprovalRequest[]>([])
  
  useEffect(() => {
    // Initialize agent
    initAgent('groq', 'llama-3.3-70b-versatile').then(setSessionId)
  }, [])
  
  useEffect(() => {
    // Poll for approvals
    const interval = setInterval(async () => {
      const approvals = await getPendingApprovals()
      setPendingApprovals(approvals)
    }, 1000)
    return () => clearInterval(interval)
  }, [sessionId])
  
  const handleApprove = async (requestId: string, allowed: boolean) => {
    await approveRequest(requestId, allowed)
    setPendingApprovals(prev => prev.filter(r => r.id !== requestId))
  }
  
  return (
    <div>
      <MessageList messages={messages} />
      {pendingApprovals.map(req => (
        <ApprovalDialog
          key={req.id}
          request={req}
          onApprove={() => handleApprove(req.id, true)}
          onDeny={() => handleApprove(req.id, false)}
        />
      ))}
    </div>
  )
}
```