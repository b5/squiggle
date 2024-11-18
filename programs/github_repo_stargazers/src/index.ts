import { log, sleep, query, addEntry, Entry, loadOrCreateSchema, Schema } from "./library";

let githubUsersSchema = {
  "title": "github_users",
  "type": "array",
  "prefixItems": [
    { "title": "id", "type": "integer", "format": "int64", "primary": true },
    { "title": "node_id", "type": "string" },
    { "title": "created_at", "type": "string", "format": "date-time" },
    { "title": "updated_at", "type": "string", "format": "date-time" },
    { "title": "login", "type": "string" },
    { "title": "gravatar_id", "type": ["string", "null"] },
    { "title": "url", "type": "string" },
    { "title": "type", "type": "string", "enum": ["User", "Organization"] },
    { "title": "site_admin", "type": "boolean" },
    { "title": "name", "type": ["string", "null"] },
    { "title": "company", "type": ["string", "null"] },
    { "title": "blog", "type": ["string", "null"] },
    { "title": "location", "type": ["string", "null"] },
    { "title": "email", "type": ["string", "null"] },
    { "title": "hireable", "type": ["boolean", "null"] },
    { "title": "bio", "type": ["string", "null"] },
    { "title": "twitter_username", "type": ["string", "null"] },
    { "title": "public_repos", "type": "integer" },
    { "title": "public_gists", "type": "integer" },
    { "title": "followers", "type": "integer" },
    { "title": "following", "type": "integer" }
  ]
};

// silly shim coming from an sqlite-backed version of this example
interface DB {
  githubUsersSchema(): Schema;
  query(schema: Schema, query: string): Entry[];
  addEntry(schema: Schema, entry: any): Entry;
}

function fetchAllStargazers(db: DB, org: string, repo: string, token: string) {
  log(`Fetching stargazers for ${org}/${repo}`);
  const perPage = 100;
  let page = 1;
  let checked = 0;

  // LOL the query string is completely ignored, just selects all from the specified table
  // so we fetch once, and pass that around
  const users = db.query(db.githubUsersSchema(), `select * from github_users`);
  if (users.length > 0) {
    log(JSON.stringify(users[0]));
  }
  // build map of updated_at by id
  const updated_at = new Map<string, string>();
  for (const user of users) {
    updated_at.set(user.id, user.data.updated_at);
  }

  while (true) {
    log(`Fetching page ${page} of stargazers`);
    const stargazers = fetchStargazersPage(org, repo, page, perPage, token);
    if (stargazers.length === 0) break;

    for (const stargazer of stargazers) {
      // if we have already checked this user, and they haven't updated since, skip
      if (updated_at.has(stargazer.id) && updated_at.get(stargazer.id) === stargazer.updated_at) {
        continue;
      }

      const user = fetchUserPage(stargazer.login, token);
      const entry = userEntry(user);
      log(`adding entry for user ${stargazer.login}`);
      db.addEntry(db.githubUsersSchema(), entry);
    }

    page++;
    checked += stargazers.length;
  }

  return checked;
}

function userEntry(res: Record<string, any>): any[] {
  return [
    res.id,
    res.created_at,
    res.updated_at,
    res.login,
    res.node_id,
    res.gravatar_id,
    res.url,
    res.type,
    res.site_admin,
    res.name,
    res.company,
    res.blog,
    res.location,
    res.email,
    res.hireable,
    res.bio,
    res.twitter_username,
    res.public_repos,
    res.public_gists,
    res.followers,
    res.following,
  ]
}

function fetchStargazersPage(org: string, repo: string, page: number, perPage: number, token: string) {
  const url = `https://api.github.com/repos/${org}/${repo}/stargazers?page=${page}&per_page=${perPage}`;
  return fetchGithubAPI(url, token);
}

function fetchUserPage(handle: string, token: string) {
  const url = `https://api.github.com/users/${handle}`;
  return fetchGithubAPI(url, token);
}

function fetchGithubAPI(url: string, token: string) {
  const req: HttpRequest = {
    url,
    method: "GET",
    headers: {
      'Accept': 'application/vnd.github.v3+json',
      'Authorization': `Bearer ${token}`,
      'X-GitHub-Api-Version': '2022-11-28'
    }
  };
  const response = Http.request(req);

  if (response.status !== 200) {
    if (response.status === 403) {
      sleep(1000 * 60 * 30); // Wait 30 minutes before retrying
      return fetchGithubAPI(url, token); 
    }
    // TODO - Handle rate limiting once we get http headers in responses:
    // https://github.com/extism/js-pdk/pull/103/files
    // if (response.status === 403 && response.headers.get('X-RateLimit-Remaining') === '0') {
    //   response.status
    //   const resetTime = response.headers.get('X-RateLimit-Reset');
    //   const delay = resetTime ? (parseInt(resetTime) * 1000 - Date.now()) : 60000; // Default to 1 minute if no reset time
    //   console.log(`Rate limit exceeded. Waiting for ${Math.round(delay / 1000)} seconds.`);
    //   await new Promise(resolve => setTimeout(resolve, delay));
    //   return fetchGithubAPI(url, token); // Retry the request
    // }

    const errorText = response.body;
    log(`Error response body: ${errorText}`);
    throw new Error(`Failed to fetch from GitHub API: ${response.status}`);
  }

  return JSON.parse(response.body);
}

function config() {
  const org = Config.get("org");
  if (!org) {
    log("missing org config value");
    throw -1;
  }
  
  const repo = Config.get("repo");
  if (!repo) {
    log("missing repo config value");
    throw -1;
  }

  const token = Config.get("github_token");
  if (!token) {
    log("missing github_token config value");
    throw -1;
  }

  return {
    org,
    repo,
    token
  }
}

export function main() {  
  const { org, repo, token } = config();

  let schema = loadOrCreateSchema(githubUsersSchema);
  let hash = (typeof schema.content === 'string') ? schema.content : schema.content.hash;
  log(`Schema loaded: ${schema.title} ${hash}`);
  log(JSON.stringify(schema));

  // build a db shim
  const db = {
    githubUsersSchema: () => schema,
    query,
    addEntry
  };

  const checked = fetchAllStargazers(db, org, repo, token);
  log(`Done! Checked ${checked} stargazers.`);
  return 0;
}