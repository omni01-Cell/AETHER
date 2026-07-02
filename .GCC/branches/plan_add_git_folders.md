# Execution Plan: Suivi de .agent/ et .GCC/ dans Git

## 📋 Target Invariant & Pre-requisites
- **Target Invariant**: Les dossiers `.agent/` et `.GCC/` doivent être suivis par Git et poussés sur GitHub afin que les workflows d'intégration continue y aient accès.
- **Pre-requisites**: Accès à Git et configuration correcte du dépôt distant.

## 🛠️ Step-by-Step Sequence

### Step 1: Retirer .GCC/ et .agent/ de .gitignore
- [ ] **Action**: Modifier `.gitignore` pour supprimer ou commenter les lignes ignorant `.GCC/` et `.agent/`.
- [ ] **Verify**: Vérifier avec `git status` que les fichiers sous ces répertoires apparaissent comme non suivis (untracked).
- **Verification Proof**:
```text
```

### Step 2: Ajouter les fichiers à l'index Git
- [ ] **Action**: Lancer `git add .gitignore package.json .husky .github .GCC .agent`
- [ ] **Verify**: Lancer `git status` pour s'assurer que tous les nouveaux fichiers et dossiers sont indexés.
- **Verification Proof**:
```text
```

### Step 3: Commiter et Pusher
- [ ] **Action**: Commiter les changements avec un message conventionnel et exécuter `git push`.
- [ ] **Verify**: Vérifier le statut de la commande de push.
- **Verification Proof**:
```text
```

## ⚠️ Mitigations & Edge Cases
- **Risk**: Le dépôt distant n'est pas configuré ou requiert des identifiants non disponibles.
- **Mitigation**: Signaler toute erreur de push immédiatement.
