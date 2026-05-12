# Git-in-Git: Using a GitHub Repo Inside a Local Git Repo

## Situation

You have a course directory that is tracked with a local Git repository, and inside that course directory you have a separate project that you want to publish to GitHub.

Example:

```text
course/
├── .git/                 # Local-only course repo
├── notes/
├── lectures/
└── github-project/
    ├── .git/             # Separate GitHub repo
    ├── src/
    └── README.md
```

## Is This a Problem?

It can work, but it should be handled intentionally.

Git does not automatically manage nested repositories the way people often expect. If a Git repository exists inside another Git repository, the outer repository will not normally track the inner repository's files in the usual way.

If you accidentally add the inner repository from the outer repository, Git may treat it as an embedded repository reference, similar to a submodule pointer. This can cause confusion because the outer repo tracks a reference to the inner repo rather than all of its actual files.

## Recommended Approach

Use two separate Git repositories:

1. The outer `course/` repo stays local.
2. The inner `github-project/` repo is pushed to GitHub.
3. The outer course repo ignores the inner GitHub project folder.

From the outer course repo:

```bash
echo "github-project/" >> .gitignore
git add .gitignore
git commit -m "Ignore standalone GitHub project"
```

Then manage the GitHub project from inside its own folder:

```bash
cd github-project
git init
git add .
git commit -m "Initial commit"
git remote add origin git@github.com:YOURNAME/YOURREPO.git
git push -u origin main
```

## Useful Safety Check

When working inside nested repositories, use this command to verify which repository Git is currently using:

```bash
git rev-parse --show-toplevel
```

This prints the root directory of the current Git repository.

## If You Already Added the Inner Repo by Mistake

From the outer course repo root:

```bash
git rm -r --cached github-project
echo "github-project/" >> .gitignore
git add .gitignore
git commit -m "Stop tracking standalone project"
```

The `--cached` option removes the folder from the outer repo's tracking without deleting it from disk.

## Alternative: Git Submodule

A submodule is useful only if you want the outer course repo to intentionally reference a specific version of the GitHub project.

Example:

```bash
cd course
git submodule add git@github.com:YOURNAME/YOURREPO.git github-project
git commit -m "Add project as submodule"
```

However, submodules add complexity and are usually unnecessary unless the course repo needs to depend on a specific commit of the project.

## Best Recommendation

For this situation, the simplest and cleanest approach is:

```text
Use a normal nested GitHub repo,
but add the project folder to the outer course repo's .gitignore.
```

This keeps the course repository local and clean while allowing the project inside it to live independently on GitHub.
