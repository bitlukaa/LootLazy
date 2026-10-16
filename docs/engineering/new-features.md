# New feature badges

Tag the feature entry with data-feature-new="feature-id-v1". The shared features module creates a localized New badge. Call window.lootlazyFeatures.markUsed("feature-id-v1") only when the feature performs its first successful action, never merely when its screen opens. Multiple entries for the same feature hide together and the used state persists locally.

Steam Accounts uses a successful authenticated connection as its first use. Adding an account, entering an invalid code or just browsing the page does not clear the badge. Future features must declare their own stable identifier and successful-use action.
