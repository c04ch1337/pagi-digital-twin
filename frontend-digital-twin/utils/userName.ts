/**
 * Utility functions for managing user name
 */

/**
 * Gets the user's display name from localStorage
 * Returns "FG_User" if no name is set
 */
export function getUserName(): string {
  const userName = localStorage.getItem('root_admin_user_name');
  return userName?.trim() || 'FG_User';
}

/**
 * Sets the user's name in localStorage
 */
export function setUserName(name: string): void {
  if (name.trim()) {
    localStorage.setItem('root_admin_user_name', name.trim());
  } else {
    localStorage.removeItem('root_admin_user_name');
  }
}
