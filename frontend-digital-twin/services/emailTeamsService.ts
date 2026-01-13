/**
 * Service for interacting with email and Teams monitoring APIs
 */

const ORCHESTRATOR_URL = import.meta.env.VITE_ORCHESTRATOR_URL || 'http://127.0.0.1:8182';

export interface EmailMessage {
  id: string;
  subject: string;
  from: {
    name?: string;
    address: string;
  };
  received_date_time: string;
  is_read: boolean;
  importance: string;
  has_attachments: boolean;
  body_preview: string;
}

export interface TeamsMessage {
  id: string;
  chat_id: string;
  channel_id?: string;
  from: {
    display_name: string;
    user_principal_name: string;
  };
  body: string;
  created_date_time: string;
  message_type: string;
  mentions: Array<{
    mention_text: string;
    mentioned: {
      display_name: string;
    };
  }>;
}

export interface EmailTrends {
  period: string;
  total_emails: number;
  unread_count: number;
  urgent_count: number;
  top_senders: Array<{
    email: string;
    name?: string;
    count: number;
  }>;
}

export interface OAuthConfig {
  client_id: string;
  client_secret: string;
  tenant_id: string;
  user_email: string;
  user_name: string;
  redirect_uri: string;
}

/**
 * Configure email/teams monitor with OAuth credentials
 */
export async function configureEmailTeams(config: OAuthConfig): Promise<{ ok: boolean; message: string }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/email-teams/configure`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(config),
  });

  if (!response.ok) {
    throw new Error(`Failed to configure email/teams: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Set OAuth tokens after authentication flow
 */
export async function setOAuthTokens(
  accessToken: string,
  refreshToken?: string
): Promise<{ ok: boolean; message: string }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/email-teams/set-tokens`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      access_token: accessToken,
      refresh_token: refreshToken,
    }),
  });

  if (!response.ok) {
    throw new Error(`Failed to set OAuth tokens: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Check for new emails
 */
export async function checkEmails(unreadOnly: boolean = true): Promise<{
  ok: boolean;
  emails: EmailMessage[];
  count: number;
}> {
  const response = await fetch(
    `${ORCHESTRATOR_URL}/api/email/check?unread=${unreadOnly}`,
    {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    }
  );

  if (!response.ok) {
    throw new Error(`Failed to check emails: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Send email reply
 */
export async function sendEmailReply(
  emailId: string,
  replyBody: string
): Promise<{ ok: boolean; message: string }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/email/send`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      email_id: emailId,
      reply_body: replyBody,
    }),
  });

  if (!response.ok) {
    throw new Error(`Failed to send email reply: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Check for new Teams messages
 */
export async function checkTeamsMessages(): Promise<{
  ok: boolean;
  messages: TeamsMessage[];
  count: number;
}> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/teams/check`, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
  });

  if (!response.ok) {
    throw new Error(`Failed to check Teams messages: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Send Teams message
 */
export async function sendTeamsMessage(
  chatId: string,
  messageContent: string
): Promise<{ ok: boolean; message: string }> {
  const response = await fetch(`${ORCHESTRATOR_URL}/api/teams/send`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      chat_id: chatId,
      message_content: messageContent,
    }),
  });

  if (!response.ok) {
    throw new Error(`Failed to send Teams message: ${response.statusText}`);
  }

  return response.json();
}

/**
 * Get email trends/statistics
 */
export async function getEmailTrends(period: 'day' | 'week' | 'month' = 'week'): Promise<{
  ok: boolean;
  trends: EmailTrends;
}> {
  const response = await fetch(
    `${ORCHESTRATOR_URL}/api/email/trends?period=${period}`,
    {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
      },
    }
  );

  if (!response.ok) {
    throw new Error(`Failed to get email trends: ${response.statusText}`);
  }

  return response.json();
}
