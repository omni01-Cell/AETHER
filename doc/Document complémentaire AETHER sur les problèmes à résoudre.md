# **AETHER — Document complémentaire : problèmes à résoudre**
![](Aspose.Words.d0bfbd82-7e72-4aa6-8ba1-da81df1963cd.001.png)

*Ce document vient en complément de la proposition initiale de l'architecture AETHER. Son but est de lister et de structurer les problèmes ouverts qu'il reste à résoudre afin de rendre le moteur réellement exploitable, précis et autonome lorsqu'il est piloté par des agents IA multimodaux.*

1. ## **Contexte**
AETHER est un moteur de création multimédia « headless » (sans interface graphique utilisateur) spécifiquement conçu pour être piloté par des agents d'intelligence artificielle. Il est capable de manipuler, de composer et de générer une grande variété de formats : vidéo, audio, image et animation. En remplaçant les interfaces logicielles traditionnelles par un protocole de commandes déclaratives, AETHER vise à supprimer la barrière d'exécution technique pour les IA créatives.

1. ## **Objectif de ce document**
Pour qu'AETHER fonctionne à un niveau de qualité professionnelle, l'agent IA doit pouvoir vérifier le fruit de son travail. Ce document vise à définir les obstacles techniques liés à la boucle de feedback (la capacité de l'IA à "voir" et "entendre" ce qu'elle produit) et à proposer des pistes de conception pour assurer un alignement parfait entre le moteur de rendu et les capacités perceptives des grands modèles de langage (LLMs) multimodaux actuels.

1. ## **Problèmes à résoudre**

### **Problème 1 : Conception d'un protocole d'observation et de validation multimodale fiable**

1. #### **Intitulé du problème**

Définir une logique de retour d'information (feedback loop) structurée permettant à un modèle multimodal d'évaluer avec précision la qualité d'un rendu vidéo généré par ses propres commandes, sans se reposer exclusivement sur l'ingestion de la vidéo brute.

1. #### **Pourquoi c’est un problème central**

Nous prévoyons de nous appuyer sur des modèles multimodaux de pointe (comme Gemini 3.1 Pro), capables d'ingérer nativement de la vidéo, des images et de l'audio. Cependant, la perception visuelle de ces modèles diffère grandement de celle d'un humain. Soumettre une vidéo brute à un modèle est insuffisant et risqué : le modèle n'a pas nécessairement une attention temporelle constante et peut manquer des artefacts de montage précis, des désynchronisations subtiles ou des erreurs de raccord. Il est impératif d'encadrer cette capacité native par une logique d'observation adaptée au moteur AETHER.

1. #### **Scénario concret d’usage**
####
Un agent IA travaille sur un montage et exécute une série de coupes. Pour valider son travail, l'agent demande au moteur de lui afficher un segment spécifique via une commande : **view -lp 3:10 - 4:30**. Le moteur doit alors renvoyer à l'agent une représentation de cet extrait lui permettant de certifier que l'enchaînement des plans est correct.

1. #### **Risques si on traite mal le problème**

0. **Hallucinations de validation :** L'agent valide un rendu défectueux car il n'a pas "vu" un saut d'image (jump cut) ou un écran noir d'une fraction de seconde.
0. **Perte de contexte :** La charge cognitive liée à l'analyse d'une vidéo brute sature la fenêtre de contexte du modèle, l'empêchant de se concentrer sur la suite du montage.
0. **Instabilité de la boucle de travail :** Sans un retour fiable, l'agent ne peut pas itérer de manière autonome.

1. #### **Hypothèse de conception**

Bien que les modèles actuels accomplissent des prouesses multimodales impressionnantes (par exemple, coder l'intégralité d'un site web à partir d'une simple capture d'écran, ou suivre des instructions complexes après avoir écouté un mémo vocal de 30 minutes), ces cas d'usage ne dispensent pas AETHER d'un protocole d'observation structuré. L'hypothèse est qu'il faut un **alignement multimodal hybride** : fournir au modèle non seulement le fichier brut, mais aussi une interprétation analytique générée par le moteur pour "diriger" l'attention de l'IA.

1. #### **Pistes de solution**

Lorsqu'un agent invoque une commande d'inspection (ex: **view**), le système ne doit pas se contenter de renvoyer le flux mp4. Le système doit renvoyer un "paquet de validation" combinant plusieurs modalités :

0. **Extrait vidéo brut :** Le clip demandé, compressé pour l'inférence.
0. **Images clés (Keyframes) :** Une planche contact extraite des moments critiques (ex: la première et la dernière image de la coupe).
0. **Audio et transcription :** La piste audio synchronisée avec les sous-titres générés (SRT) pour vérifier les coupes sur les dialogues.
0. **Métadonnées et résumés structurés :** Un fichier JSON décrivant techniquement le segment (nombre de plans, détection d'écrans noirs, niveaux de volume RMS).
0. **Événements détectés :** Alertes pré-calculées par le moteur (ex: "Attention, le niveau audio sature à 3:15").

1. #### **Expériences à mener**

0. **Test de cécité temporelle :** Introduire volontairement des erreurs de montage (glitch, flash noir de 2 frames) dans un rendu et mesurer si Gemini 3.1 Pro les détecte via la vidéo brute vs. via le paquet de validation structuré.
0. **Équilibrage de la charge modale :** Mesurer le coût en tokens et le temps de latence en comparant l'envoi d'une vidéo complète avec l'envoi de 5 keyframes accompagnés d'un graphe audio JSON.
0. **Évaluation du prompt d'inspection :** Rédiger et tester des "system prompts" spécifiques qui obligent le modèle à croiser la vidéo avec les métadonnées fournies avant d'émettre un jugement sur son montage.

1. #### **Critères de réussite**

0. Le modèle identifie 95 % des anomalies techniques introduites dans un segment de 30 secondes.
0. Le modèle est capable de corriger de lui-même une commande de découpe erronée (**trim**) après avoir reçu le retour d'observation.
0. Le temps de réponse de la boucle d'observation (génération du paquet + inférence du modèle) reste inférieur à 10 secondes.

1. #### **Questions ouvertes**

0. Quel format de retour multimodal offre le meilleur compromis entre fiabilité perceptive, coût d'inférence et latence ?
0. Quels types d'erreurs vidéo les modèles multimodaux détectent mal lorsqu'ils ne reçoivent qu'un clip brut ?
0. Faut-il distinguer plusieurs modes d'inspection selon l'objectif : contrôle visuel, validation narrative, contrôle audio, détection d'artefacts techniques ?
0. Quelle part de l'analyse doit être faite par le moteur AETHER avant envoi au modèle, et quelle part doit rester déléguée au modèle multimodal principal ?

### **Problème 2 : Gestion de l’état persistant et cohérence du projet entre les tours d’agent**

1. #### **Pourquoi c’est un vrai problème**

在 AETHER 的设想中，Agent 并不是一次性完成所有创作，而是通过“计划 → 执行 → 观察 → 修正”的循环持续工作。这意味着系统必须长期维护项目状态，包括时间线、素材引用、渲染结果、历史操作、失败任务和分支版本。如果状态管理不稳定，Agent 在第 N 次调用时看到的项目状态可能与第 N-1 次执行后的真实状态不一致，进而导致错误剪辑、重复操作、覆盖正确结果，甚至把项目推入不可恢复的混乱状态。

这不是普通软件里的“小型同步问题”，而是 AETHER 能否成为“可迭代创作引擎”的基础问题。因为对 Agent 来说，项目状态就是它的外部记忆；如果这个外部记忆不可靠，整个系统的推理链条就会被破坏。

1. #### **Pourquoi c’est important**

只有当状态、引用、快照和实际媒体结果始终一致时，Agent 才能像人类编辑一样逐步推进复杂项目。否则，AETHER 将退化为一组零散命令，而不是一个真正可持续工作的创作操作系统。
### **Problème 3 : Définition d’un modèle de référence stable pour les assets, segments et objets éditoriaux**

1. #### **Pourquoi c’est un vrai problème**

AETHER 强调使用类似 **@v1**、**@a1** 这样的短引用，让模型以低成本操作复杂媒体对象。但在真实生产场景里，媒体对象会不断变化：视频会被裁切、音频会被替换、时间线会被重排、生成内容会出现多个候选版本。如果引用系统只是“看起来简单”，却不能清晰表达对象身份、版本关系、时间范围和派生链路，Agent 很快就会把“原始素材”“裁切片段”“当前版本”“历史版本”混淆。

一旦引用语义不稳定，所有高层 DSL 命令都会变得脆弱，因为模型会在错误的对象上继续编辑。问题不只是命名，而是“编辑对象身份模型”本身是否足够严谨。

1. #### **Pourquoi c’est important**

稳定的引用系统决定了 Agent 是否能安全地进行多轮编辑、回滚、比较、分支合并和结果复用。没有这一层，AETHER 的“低上下文、高可控”优势很难真正成立。
### **Problème 4 : Contrôle de l’atomicité, de l’annulation et de la reprise après erreur**

1. #### **Pourquoi c’est un vrai problème**

AETHER 的命令会跨越多个子系统：视频处理、音频处理、素材管理、渲染队列、外部生成 API、项目数据库等。一个看似简单的命令，背后可能触发多步异步操作。如果其中一步失败，而系统没有原子性保障，项目就可能处于“部分成功、部分失败”的中间状态。例如时间线已经更新，但代理视频尚未生成；数据库记录已写入，但实际文件不存在；或外部 API 已返回结果，但本地索引未同步。

对人类用户来说，这类问题已经很难处理；对 Agent 来说更危险，因为模型往往会基于“系统返回成功”继续规划下一步，而不会主动怀疑底层状态是否半损坏。

1. #### **Pourquoi c’est important**

如果没有事务性、可回滚、可恢复的执行模型，AETHER  将难以支持可靠自动化。一个真正面向

Agent 的创作引擎，不能只关注“能执行”，还必须保证“失败后可解释、可恢复、可继续”。

### **Problème 5 : Orchestration des coûts, de la latence et des quotas des modèles externes**

1. #### **Pourquoi c’est un vrai problème**

AETHER 依赖多模态模型和生成式 API 才能实现其完整愿景，但这些能力几乎都伴随着显著成本：调用费用、速率限制、并发限制、响应波动、任务排队时间以及供应商侧的不确定性。如果系统不主动管理这些资源，Agent 很可能在一次复杂创作循环里频繁调用高成本能力，例如重复生成视频、反复请求长片段分析、对多个候选版本都做全量检查，最终导致成本失控、响应过慢，甚至被 API 限流中断。

这类问题在 Demo 阶段往往不明显，但一旦进入持续创作或多项目并发，就会立刻成为系统级瓶颈。AETHER 不能假设“模型能力可无限使用”，而必须把成本与延迟当作一等设计约束。

1. #### **Pourquoi c’est important**

成本与延迟控制直接决定 AETHER 是否具备商业可行性和工程可运营性。若每一次观察、渲染、生成都过于昂贵或缓慢，Agent 就无法稳定形成高频迭代闭环。
### **Problème 6 : Évaluation de la qualité finale et arbitrage entre critères parfois contradictoires**

1. #### **Pourquoi c’est un vrai problème**

AETHER 不只是执行技术命令，它的目标是帮助 Agent 产出“足够好”的成品。但“好”本身并不是单一指标：一段视频可能技术上无错误，却节奏拖沓；一段旁白可能内容准确，却情感不匹配；一段动画可能叙事清楚，却风格不统一。对 Agent 来说，如果系统只返回技术正确性，而缺乏对叙事、

节奏、视觉一致性、音画关系等高层质量维度的评估支持，那么它仍然很难做出接近人类制作人的判断。

更复杂的是，这些指标常常互相冲突：更清晰的字幕可能破坏画面构图，更强的压缩可能提升速度却损害质感，更短的剪辑可能提升节奏却牺牲信息完整性。系统必须面对“多目标质量优化”的现实，而不是假设存在单一正确答案。
1. #### **Pourquoi c’est important**

如果 AETHER 无法帮助 Agent 进行质量判断与权衡，它就只能停留在“媒体自动化工具”层面，而难以支持真正意义上的创作决策。
### **Problème 7 : Sécurité, permissions et isolement d’un moteur piloté par agent**

1. #### **Pourquoi c’est un vrai problème**

AETHER 被设计为一个可由 Agent 直接驱动的 headless 引擎，这意味着命令执行面会比传统 GUI软件更开放。Agent 可能访问本地文件、调用外部 API、写入项目目录、触发长时间渲染、下载或引用第三方素材。如果权限边界不明确，系统就可能出现越权访问、错误删除、敏感密钥泄露、恶意素材注入或非预期资源消耗等问题。

这一问题的重要性在于：AETHER 越强大，Agent 的错误操作潜在破坏面就越大。系统不能仅依赖

“模型应该谨慎”这种软约束，而需要在架构层建立明确的权限、沙箱、审计和策略控制。

1. #### **Pourquoi c’est important**

没有安全边界的 Agent 创作系统很难进入真实生产环境。对团队、企业或平台而言，安全与可审计性不是附加项，而是部署前提。
### **Problème 8 : Interopérabilité avec les workflows professionnels et export vers d’autres outils**

1. #### **Pourquoi c’est un vrai problème**

AETHER 的愿景不是孤立存在，它最终需要与真实制作流程协同：导入素材库、导出给人工编辑器继续微调、接入品牌资产系统、与字幕/配音/审校流程衔接，甚至转换为行业常见时间线格式。若 AETHER 生成的项目结构只适用于自身内部，而无法稳定映射到外部工具和标准，团队就会被锁定在一个封闭流程里，导致 adoption 成本上升。

此外，Agent 生成的结果通常不会百分之百直接上线，很多情况下仍需人工校正和专业软件二次处理。因此，系统必须从一开始就考虑“如何交接给人类与其他软件”，而不是只考虑内部闭环。

1. #### **Pourquoi c’est important**

互操作性决定了 AETHER 是一个能嵌入行业流程的基础设施，还是一个只能展示概念的孤岛产品。对产品落地而言，这一差异非常关键。
### **Problème	9	:	Mesure	expérimentale	des	capacités	réelles	des	modèles multimodaux dans le contexte AETHER**

1. #### **Pourquoi c’est un vrai problème**
   ####
AETHER 的很多设计假设都建立在“多模态模型已经足够强”这一判断之上。但模型在公开演示中的能力，并不自动等于它在 AETHER 工作流中的可靠性。能根据一张图片复刻网页、能听 30 分钟语音并执行指令，并不代表它就能稳定发现 2 帧黑屏、理解复杂时间线语义、比较两个近似版本的节奏差异，或在多轮编辑后保持一致判断。

因此，AETHER 不能只依据通用印象来设计系统，而必须建立一套针对自身任务的实验基线：哪些能力真实可靠，哪些能力只在特定提示词下有效，哪些能力需要系统侧补偿。否则，架构会建立在未经验证的乐观假设上。
1. #### **Pourquoi c’est important**

没有实验基线， 就无法判断该把责任放在模型、提示词、工具协议还是前处理逻辑上。对

AETHER 这种以 Agent 能力为核心前提的系统来说，能力测量本身就是核心工程工作。

1. ## **Questions ouvertes / prochaines expériences**
Afin de poursuivre la définition de l'architecture, les questions suivantes devront faire l'objet de nos prochains ateliers techniques :

- Comment compresser au mieux le signal audio pour qu'il soit analysé sémantiquement par l'agent sans surcharger le contexte ?
- Devons-nous utiliser un modèle de vision secondaire (plus petit, tournant localement sur le Daemon AETHER) pour pré-filtrer les erreurs visuelles avant même de solliciter le LLM principal ?
- Quelle est la densité optimale d'images clés (keyframes par seconde) à fournir au modèle pour garantir une perception temporelle sans faille ?
- Quel niveau de granularité faut-il donner aux références d’objets pour permettre à la fois la simplicité du DSL et la précision éditoriale ?
- Quel protocole d’exécution permet de reprendre automatiquement un projet après échec partiel sans corrompre la timeline ni l’historique ?
- Comment définir un budget d’inférence par tâche pour éviter qu’un agent n’utilise de manière excessive les modèles les plus coûteux ?
- Quels critères de qualité doivent être évalués automatiquement, et lesquels doivent rester validés par un humain ?
- Jusqu’où AETHER doit-il aller dans l’export vers des standards externes comme OpenTimelineIO avant que cela ne complexifie excessivement le cœur du moteur ?
