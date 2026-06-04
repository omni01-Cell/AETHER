# **Guide d'Ingénierie Avancée : Famille Nano Banana (Gemini 3 Image)**

## **Introduction**

La famille Nano Banana (basée sur l'architecture Gemini 3, incluant les modèles Flash et Pro) représente l'état de l'art de Google en matière de génération et d'édition d'images. Ce document ne se contente pas de vous apprendre à "parler" au modèle ; il vous explique comment manipuler sa mécanique interne (Tokenizer et Weights) pour obtenir des résultats professionnels, constants et précis.

## **1\. La Règle d'Or du Tokenizer : L'Anglais par défaut**

Arrêtez de gaspiller l'attention du modèle. Un modèle IA ne lit pas des mots, il lit des **tokens** (fragments de mots).

En raison de la manière dont les modèles sont entraînés, l'anglais est massivement plus efficace. Pour un même concept, le français ou d'autres langues peuvent consommer **2 à 3 fois plus de tokens**.

* **Le problème :** Plus votre prompt génère de tokens, plus l'attention du modèle se dilue (phénomène de "bruit"). Les détails fins de votre prompt risquent d'être ignorés.  
* **La solution :** Rédigez **toujours** la structure de base de votre prompt (sujet, environnement, spécifications techniques de caméra, éclairage) en **anglais**.

## **2\. Le "Weight Hacking" : L'usage stratégique des langues locales**

Si l'anglais est le squelette, les langues locales sont des clés d'accès aux **poids culturels (cultural weights)** du modèle. L'IA a encodé des concepts différemment selon la langue du dataset d'entraînement. Utiliser une langue spécifique force le modèle à "réfléchir" avec ces biais culturels.

### **Quand utiliser une autre langue que l'anglais ?**

1. **Génération de texte intégré précis (OCR localisé) :**  
   * *Mauvais (Anglais pour du FR) :* A blackboard with green text that reads: "vente de prototype ia" (L'IA tente de traiter le français avec sa logique anglaise, générant des fautes).  
   * *Excellent (Français direct) :* Un tableau noir avec du texte en couleur verte qui affiche : "vente de prototype ia"  
2. **Activation d'associations culturelles profondes (Esthétique) :**  
   * *Exemple K-Pop :* Si vous demandez "K-pop idol" en anglais, le modèle puise dans un dataset occidental stéréotypé. Si vous intégrez des termes en **coréen**, vous activez les poids d'entraînement liés aux médias natifs coréens, modifiant radicalement la justesse des traits, des vêtements et de l'ambiance.  
   * *Exemple de l'approche psychologique :* Tout comme demander à un modèle IA de "réfléchir en japonais" pour une tâche d'analyse stratégique l'oblige à adopter des pondérations liées à la prudence et aux conséquences (issues de la culture japonaise), utiliser le japonais pour générer une ruelle cyberpunk donnera un résultat plus authentique qu'une description anglophone d'une "Tokyo street".

**La Stratégie Hybride Optimale :** L'anglais pour la cinématographie et l'action \+ La langue cible *uniquement* pour le texte à afficher ou l'élément culturel clé.

## **3\. Structure d'un prompt optimal (La règle des 40-80 mots)**

**Cible : 40 à 80 mots** (soit environ 50 à 90 tokens en anglais).

* *\< 20 mots :* Manque de contexte, l'IA remplit les vides avec des stéréotypes (moyennes statistiques).  
* *\> 100 mots :* Dilution de l'attention. L'IA oublie le début du prompt ou mélange les concepts.

**Évitez le "Tag Soup" :** Les listes de mots séparés par des virgules (dog, park, 4k, unreal engine) sont obsolètes. Faites des phrases descriptives liant les concepts.

### **Les 4 couches d'ingénierie (en Anglais) :**

1. **Sujet \+ Action (15-20 mots) :** Qui fait quoi ?*"A stoic robot barista with glowing blue optics brewing an espresso..."*  
2. **Environnement (15-20 mots) :** Où ?*"...inside a futuristic cafe on Mars with red dust outside the window..."*  
3. **Éclairage \+ Ambiance (10-20 mots) :** Quelle atmosphère ?*"...golden hour backlighting creating long shadows, cinematic haze..."*  
4. **Spécifications techniques (10 mots) :** Rendu et caméra ?*"...shot on 35mm lens, shallow depth of field (f/1.8), 21:9 cinematic aspect ratio."*

## **4\. Le Conversational Editing (Édition Conversationnelle)**

Ne relancez jamais un nouveau prompt "from scratch" si votre image est à 80% correcte. La famille Nano Banana maintient un contexte conversationnel.

* **Soyez direct :** *"Change the man's tie to green"* ou *"Remove the car in the background"*.  
* **Ajustement de studio :** *"It's perfect, but change the lighting to a sunset and make the text neon blue."*

## **5\. Capacités Avancées : Multi-images et Consistance**

### **A. Cohérence de Personnages (Character Lock & Image Blending)**

Nano Banana peut ingérer jusqu'à 14 images (selon les plateformes) et maintenir la consistance faciale pour plusieurs personnages simultanément.

* **La méthode de l'attribution :** Ne jetez pas des images au hasard. Assignez des rôles explicites dans votre prompt.*"Use Image A for the character's face, Image B for the watercolor illustration style, and Image C for the background environment."*

### **B. Contrôle du Branding et de l'Identité**

Vous pouvez draper des logos, motifs ou typographies sur des objets 3D (vêtements, emballages) de manière organique, en préservant l'éclairage volumétrique et les textures (le logo épousera les plis d'un t-shirt).

### **C. Factualité et Contraintes (Diagrammes)**

Nano Banana (spécialement le modèle Pro) intègre la connaissance du monde réel de Gemini 3\. Pour générer des diagrammes, spécifiez la contrainte de véracité :

* *"A scientifically accurate cross-section diagram of a human heart."*

## **6\. Problèmes fréquents et Dépannage Bas Niveau**

| Problème | Cause Mécanique | Solution |
| :---- | :---- | :---- |
| **Ignorance de détails** | Trop de tokens (prompt trop long) ou dilution dans une langue gourmande (Français). | Passez la description technique en anglais. Restez sous la barre des 80 mots. |
| **Texte généré illisible** | Le modèle n'a pas de contrainte de surface de rendu. | Définissez le layout : *"The headline 'URBAN' rendered in bold white sans-serif at the top center."* Utilisez la langue cible. |
| **Image "plate" ou clichée** | Prompt sans spécifications de caméra. L'IA choisit l'angle moyen statistique. | Forcer des valeurs extrêmes/précises : *"Extreme low-angle shot, macro lens, volumetric lighting."* |
| **Dégradation après 5 éditions** | L'encodage se détériore à chaque itération successive sur la même image. | Définissez les réglages lourds (angle, lumière) dès le prompt 1\. Faites des ajustements mineurs ensuite. |

## **7\. Limitations Actuelles à garder en tête**

* **Fidélité typographique extrême :** Les tout petits textes ou les paragraphes entiers risquent de comporter des hallucinations orthographiques.  
* **Artefacts de fusion :** Demander de fusionner des concepts physiquement impossibles avec 14 images sources peut générer des géométries non-euclidiennes ou des artefacts d'éclairage.  
* **Traductions complexes :** Générer une affiche multilingue peut parfois rater des nuances culturelles si vous n'avez pas injecté un prompt en langue locale pour forcer le *Weight Hacking*.