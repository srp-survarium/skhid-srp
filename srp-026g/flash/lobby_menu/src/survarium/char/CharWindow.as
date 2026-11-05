package survarium.char
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import flash.external.ExternalInterface;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.StatusIndicator;
   import scaleform.clik.controls.TileListSmoothR;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   import survarium.Util;
   import survarium.constants.Events;
   import survarium.controls.Message;
   import survarium.controls.UIWindow;
   import survarium.data.LocalizationData;
   import survarium.data.PropertiesDescriptor;
   import survarium.interfaces.ISkillRenderer;
   import survarium.constants.PropertyDirection;
   
   public class CharWindow extends UIWindow
   {
      
      protected const SKILL_LEVELS:uint = 25;
      
      protected const POINTS_PER_LEVEL:uint = 5;
      
      // a skill branch's tiers (one every POINTS_PER_LEVEL branch points) and the points of its last
      private static const TIERS:int = 5;
      
      private static const LAST_TIER_POINTS:int = 20;
      
      protected var _data:Object;
      
      protected var _enterName:MovieClip;
      
      protected var _treesData:Array = new Array();
      
      protected var _branches:Vector.<*> = new Vector.<*>();
      
      protected var _branchesDict:Dictionary = new Dictionary();
      
      protected var _reroll_cost:uint = 0;
      
      protected var _reroll_cost_premium:uint = 0;
      
      protected var _perkItems:Array = [];
      
      protected var _charStats:DataProvider;
      
      protected var _startStats:Array = [];
      
      protected var _skillContainer:Sprite = new Sprite();
      
      protected var _skillRenderers:Array = new Array();
      
      protected var _skillClassRenderer:Class;
      
      public var points_aviable:int = 0;
      
      public var progressIndicator:StatusIndicator;
      
      public var level_progress_bar:MovieClip;
      
      public var backgrond:MovieClip;
      
      public var _left:MovieClip;
      
      public var _rith:MovieClip;
      
      public var btn_set:Button;
      
      public var btn_reroll:Button;
      
      public var btn_cancel:Button;
      
      public var avatarList:MovieClip;
      
      public var titulList:MovieClip;
      
      public var btn_change_name:Button;
      
      public var btn_change_titul:Button;
      
      public var skillsTabPage:ButtonBar;
      
      public var all_factions:Button;
      
      public var _klan:TextField;
      
      public var _rpvp:TextField;
      
      public var _rpve:TextField;
      
      public var _max_xp:TextField;
      
      public var _week_pvp:TextField;
      
      public var _week_pve:TextField;
      
      public var _week_xp:TextField;
      
      public var _last_achiv:TextField;
      
      public var _total_achiv:TextField;
      
      public var point_available:TextField;
      
      public var point_available_value:TextField;
      
      public var label:TextField;
      
      private var _skillsPropDict:Dictionary = new Dictionary();
      
      private var _skillsArray:Array = [];
      
      private var _skillsProgressArray:Array = [];
      
      public var selectedTabGlow:MovieClip;
      
      private var _activeSkillBranch:int = -1;
      
      private var _skillList:Array;
      
      protected var _propsDescriptor:PropertiesDescriptor;
      
      private var newSkills:Array = [];
      
      public function CharWindow()
      {
         super();
         addEventListener(Event.RENDER,this.init);
         trace("CharWindow::",btnClose,btnClose2,this.btn_set);
      }
      
      override public function get height() : Number
      {
         return 902;
      }
      
      override public function get width() : Number
      {
         return 580;
      }
      
      public function registerRenderer(value:Class) : void
      {
         _skillClassRenderer = value;
      }
      
      public function set reroll_cost(value:int) : void
      {
         _reroll_cost = value;
      }
      
      public function set reroll_cost_premium(value:int) : void
      {
         _reroll_cost_premium = value;
      }
      
      public function get reroll_cost() : int
      {
         return this._reroll_cost;
      }
      
      public function get reroll_cost_premium() : int
      {
         return this._reroll_cost_premium;
      }
      
      public function init(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.init);
         trace("CWinit",this.btn_reroll,this.btn_set,this.btn_reroll,this.btn_cancel,this.skillsTabPage);
         this.btn_reroll.enabled = true;
         LocalizationData.addTextField(this.point_available,"points_aviable");
         LocalizationData.addTextField(this.label,"abilities");
         LocalizationData.addCallback(this.onLocal,"reroll");
         this.btn_reroll.tooltipRendererName = "SrvProfileTipRenderer";
         this.btn_set.addEventListener(ButtonEvent.CLICK,this.saveChanges);
         this.btn_reroll.addEventListener(ButtonEvent.CLICK,this.rerollChanges);
         this.btn_cancel.addEventListener(ButtonEvent.CLICK,this.cancelChanges);
         if(this._rith)
         {
            this._rith.addEventListener(ButtonEvent.CLICK,this.scrollAchiv);
         }
         if(this._left)
         {
            this._left.addEventListener(ButtonEvent.CLICK,this.scrollAchiv);
         }
         if(this.avatarList)
         {
            this.avatarList.btn_set.addEventListener(MouseEvent.CLICK,this.setNewAvatar);
            this.avatarList.btn_cancel.addEventListener(MouseEvent.CLICK,this.hideAvatars);
            this.avatarList.visible = false;
         }
         if(this.titulList)
         {
            this.titulList.visible = false;
            this.titulList.btn_set.addEventListener(MouseEvent.CLICK,this.setNewTitul);
            this.titulList.btn_cancel.addEventListener(MouseEvent.CLICK,this.hideTituls);
         }
         this.skillsTabPage.addEventListener(IndexEvent.INDEX_CHANGE,this.updateSkillsData);
         this._skillContainer.addEventListener(Events.SKILL_UPDATE,this.onSkillUpdate);
         this._skillContainer.addEventListener(Events.SHOW_ANTISKILLS,this.onSkillAntiskills);
         this._skillContainer.addEventListener(Events.HIDE_ANTISKILLS,this.onSkillHideAntiskills);
         if(this.btn_change_titul)
         {
            this.btn_change_titul.addEventListener(MouseEvent.CLICK,this.showTitulList);
         }
         var temp_achievments:Array = [];
         for(var i:uint = 0; i < 20; i++)
         {
            var icon:MovieClip = new MovieClip();
            icon.gotoAndStop(Math.random() * 2);
            temp_achievments.push({
               label:"achievement" + i.toString(),
               icon:icon
            });
         }
         this.skillsTabPage.dataProvider = new DataProvider([{
            name:"skills_01",
            id:0,
            index:0,
            icon:1
         },{
            name:"skills_02",
            id:1,
            index:1,
            icon:2
         },{
            name:"skills_03",
            id:2,
            index:2,
            icon:3
         },{
            name:"skills_04",
            id:3,
            index:3,
            icon:4
         },{
            name:"skills_05",
            id:4,
            index:4,
            icon:5
         }]);
         addChild(this._skillContainer);
      }
      
      private function onLocal(value:String) : void
      {
         this.btn_reroll.data = {name:value};
         this.btn_set.label = LocalizationData.getName("confirm").toUpperCase();
         this.btn_cancel.label = LocalizationData.getName("cancel").toUpperCase();
      }
      
      private function onSkillHideAntiskills(e:Event) : void
      {
         var antiskills:Array = e.target.data.antiskills;
         var l:uint = this._skillRenderers.length;
         var l_antiskills:uint = antiskills.length;
         for(var j:uint = 0; j < l_antiskills; j++)
         {
            for(var i:uint = 0; i < l; i++)
            {
               var renderer:MovieClip = this._skillRenderers[i];
               var rendererData:Object = renderer.data;
               if(rendererData.id == antiskills[j])
               {
                  renderer.showLock(false);
               }
            }
         }
      }
      
      private function handleAntiskills() : void
      {
         trace("handleAntiskills");
         var l:uint = this._skillRenderers.length;
         for(var i:uint = 0; i < l; i++)
         {
            var renderer:MovieClip = this._skillRenderers[i];
            var rendererData:Object = renderer.data;
            var antiskills:Array = rendererData.antiskills;
            if(antiskills && renderer.enabled)
            {
               var l_antiskills:uint = antiskills.length;
               for(var j:uint = 0; j < l_antiskills; j++)
               {
                  var rendererToLock:MovieClip = this.getSkillRenderer(antiskills[j]);
                  if(rendererToLock && rendererToLock.enabled)
                  {
                     rendererToLock.lock(Boolean(rendererData.newValue > 0));
                  }
               }
            }
         }
      }
      
      private function getSkillRenderer(id:uint) : MovieClip
      {
         var l:uint = this._skillRenderers.length;
         for(var i:uint = 0; i < l; i++)
         {
            var renderer:MovieClip = this._skillRenderers[i];
            var rendererData:Object = renderer.data;
            if(rendererData.id == id)
            {
               return renderer;
            }
         }
         return null;
      }
      
      private function onSkillAntiskills(e:Event) : void
      {
         var antiskills:Array = e.target.data.antiskills;
         trace("onSkillAntiskills",antiskills);
         var l:uint = this._skillRenderers.length;
         var l_antiskills:uint = antiskills.length;
         for(var j:uint = 0; j < l_antiskills; j++)
         {
            for(var i:uint = 0; i < l; i++)
            {
               var renderer:MovieClip = this._skillRenderers[i];
               var rendererData:Object = renderer.data;
               if(rendererData.id == antiskills[j])
               {
                  renderer.showLock(true);
               }
            }
         }
      }
      
      private function onSkillUpdate(e:Event) : void
      {
         this.btn_set.enabled = true;
         this.btn_cancel.enabled = true;
         var branchPoints:int = (this.getBranchPoints());
         var spendPoints:int = (this.getSpendPoints());
         trace("onSkillUpdate",this._data.points_available,spendPoints);
         this.setPoints(this._data.points_available - spendPoints);
         this.setSkillProgress(branchPoints);
         this.handleAntiskills();
         this.checkButton();
      }
      
      private function setSkillProgress(branchPoints:uint) : void
      {
         this.progressIndicator.value = Math.min(branchPoints + 1,this.progressIndicator.maximum - this.POINTS_PER_LEVEL);
         var unclockedData:Object = this.getBranchUnlockedLevel();
         var level:uint = unclockedData.level * this.POINTS_PER_LEVEL;
         if(unclockedData.lastUnlocked == true)
         {
            this.progressIndicator.value = this.progressIndicator.maximum;
         }
         trace("setSkillProgress::",branchPoints,level,"|",this.points_aviable);
         var l:uint = this._skillRenderers.length;
         for(var i:uint = 0; i < l; i++)
         {
            var renderer:ISkillRenderer = this._skillRenderers[i];
            var rendererData:Object = renderer.data;
            renderer.enabled = rendererData.branch_points <= level;
            renderer.unlockable = this.points_aviable > 0;
            renderer.invalidateData();
         }
      }
      
      private function getBranchUnlockedLevel() : Object
      {
         var _currentSkills:Array = this._skillList[this._activeSkillBranch];
         _currentSkills.sortOn(["level","id"],Array.NUMERIC);
         var l:uint = _currentSkills.length;
         var data:Object = {};
         var value:uint = 0;
         var level:uint = 0;
         var lastUnlocked:Boolean = false;
         for(var i:uint = 0; i < l; i++)
         {
            var skillData:Object = _currentSkills[i];
            trace("getBranchUnlockedLevel",skillData.id,skillData.branch_points,skillData.newValue);
            if(skillData && skillData.branch_points <= this.SKILL_LEVELS)
            {
               if(skillData.branch_points <= value)
               {
                  level = (int(skillData.branch_points / this.POINTS_PER_LEVEL));
                  if(skillData.branch_points == LAST_TIER_POINTS && skillData.newValue > 0)
                  {
                     lastUnlocked = true;
                  }
               }
               else
               {
                  break;
               }
               value += skillData.newValue;
            }
         }
         level = Math.min(TIERS,level);
         data.level = level;
         data.lastUnlocked = lastUnlocked;
         return data;
      }
      
      private function getSpendPoints() : uint
      {
         var value:uint = 0;
         for(var j:uint = 0; j < this._skillList.length; j++)
         {
            var _currentSkills:Array = this._skillList[j];
            if(!_currentSkills)
            {
               continue;
            }
            {
               var l:uint = _currentSkills.length;
               for(var i:uint = 0; i < l; i++)
               {
                  var skillData:Object = _currentSkills[i];
                  if(skillData)
                  {
                     value += skillData.newValue - skillData.value;
                  }
               }
            }
         }
         return value;
      }
      
      private function getBranchPoints() : uint
      {
         var _currentSkills:Array = this._skillList[this._activeSkillBranch];
         var l:uint = _currentSkills.length;
         var value:uint = 0;
         for(var i:uint = 0; i < l; i++)
         {
            var skillData:Object = _currentSkills[i];
            if(skillData)
            {
               value += skillData.newValue;
            }
         }
         return value;
      }
      
      public function fillSkillsProperties(value:Array) : void
      {
         trace("fillSkillsProperties");
         var l:uint = value.length;
         _skillsArray = new Array();
         var branchEnabled:Object = new Object();
         for(var i:uint = 0; i < l; i++)
         {
            var skillData:Object = value[i];
            Util.logObject(skillData);
            this._skillsPropDict[skillData.id] = skillData;
            SkillNames.record(skillData.id,skillData.name);
            branchEnabled[skillData.branch] = true;
            this._skillsArray[i] = {
               id:skillData.id,
               branch:skillData.branch,
               value:0,
               maxValue:skillData.max_value,
               enabled:true,
               name:skillData.name,
               description_1:skillData.description_1,
               description_2:skillData.description_2
            };
         }
         if(this.skillsTabPage.dataProvider)
         {
            for(i = 0; i < this.skillsTabPage.dataProvider.length; i++)
            {
               if(this.skillsTabPage && this.skillsTabPage.dataProvider[i])
               {
                  this.skillsTabPage.dataProvider[i].enabled = branchEnabled[i] == true;
               }
            }
            this.skillsTabPage.dataProvider.invalidate();
         }
         this.fillSkills(this._skillsArray);
      }
      
      public function fillSkillsProgress(value:Array) : void
      {
         _skillsProgressArray = value;
         var progressData:Object = this._skillsProgressArray[this._activeSkillBranch];
         if(progressData)
         {
            this.progressIndicator.maximum = this.SKILL_LEVELS + 1;
            this.setSkillProgress(progressData.points);
         }
         trace("fillSkillsProgress");
         var skillsProvider:DataProvider = this.skillsTabPage.dataProvider as DataProvider;
         if(skillsProvider)
         {
            for(var i:uint = 0; i < value.length; i++)
            {
               progressData = value[i];
               Util.logObject(progressData);
               for(var j:uint = 0; j < skillsProvider.length; j++)
               {
                  if(skillsProvider[j].id == progressData.id)
                  {
                     this.skillsTabPage.dataProvider[j].name = progressData.name;
                  }
               }
            }
            this.skillsTabPage.dataProvider.invalidate();
         }
      }
      
      public function fillSkills(value:Array) : void
      {
         trace("fillSkills:",this.points_aviable);
         _skillsArray = value;
         _skillList = [[],[],[],[],[]];
         var l:uint = this._skillsArray.length;
         for(var i:uint = 0; i < l; i++)
         {
            var skillData:Object = this._skillsArray[i];
            Util.logObject(skillData);
            if(skillData)
            {
               var skillProp:Object = this._skillsPropDict[skillData.id];
               var skillTab:int = skillProp.branch == undefined ? (uint(skillData.id / 100) - 1) : (skillProp.branch);
               trace("Skill::fillSkill",skillData.id,skillData.value,skillData.enabled,skillTab);
               if(skillTab < 0 || skillTab > this._skillList.length - 1)
               {
                  trace("wrongSkillTab");
               }
               else
               {
                  skillData.newValue = skillData.value;
                  skillData.maxValue = skillProp.max_value;
                  skillData.name = skillProp.name;
                  skillData.branch = skillProp.branch;
                  skillData.description_1 = skillProp.description_1;
                  skillData.description_2 = skillProp.description_2;
                  skillData.icon = skillProp.icon;
                  skillData.level = skillProp.row;
                  skillData.max_value = skillProp.max_value;
                  skillData.antiskills = skillProp.antiskills;
                  skillData.branch_points = skillProp.branch_points;
                  this._skillList[skillTab].push(skillData);
               }
            }
         }
         if(this.skillsTabPage.selectedIndex == -1)
         {
            this.skillsTabPage.selectedIndex = 0;
         }
         else
         {
            this.fillSkillsList(this._activeSkillBranch);
         }
      }
      
      private function updateSkillsData(e:IndexEvent) : void
      {
         _activeSkillBranch = e.index;
         this.selectedTabGlow.width = 214;
         this.selectedTabGlow.mouseEnabled = false;
         this.selectedTabGlow.x = this.skillsTabPage.x + this.skillsTabPage.buttonWidth * e.index - 50;
         this.fillSkillsList(this._activeSkillBranch);
      }
      
      private function fillSkillsList(index:int) : void
      {
         if(!this._skillList || index == -1)
         {
            return;
         }
         var _currentSkills:Array = this._skillList[index];
         this._skillContainer.x = 50;
         this._skillContainer.y = 180;
         var RENDERER_STEP_H:uint = 120;
         var RENDERER_STEP_V:uint = 135;
         var renderer:MovieClip;
         var l:uint = _currentSkills.length;
         trace("fillSkillsList",index);
         for(var i:uint = 0; i < l; i++)
         {
            var skillData:Object = _currentSkills[i];
            if(!this._skillRenderers[i])
            {
               trace("fillSkillsList::createRenderer");
               this._skillRenderers[i] = new this._skillClassRenderer();
            }
            renderer = this._skillRenderers[i];
            if(!renderer.parent)
            {
               this._skillContainer.addChild(renderer);
            }
            var skillProp:Object = this._skillsPropDict[skillData.id];
            renderer.x = RENDERER_STEP_H * (skillProp.column - 1);
            renderer.y = RENDERER_STEP_V * (skillProp.row - 1);
            var skillRenderer:ISkillRenderer = this._skillRenderers[i];
            skillRenderer.data = skillData;
            skillRenderer.invalidateData();
            skillRenderer.lock(false);
         }
         while(i < this._skillRenderers.length)
         {
            renderer = this._skillRenderers[i];
            if(this._skillContainer.contains(renderer))
            {
               this._skillContainer.removeChild(renderer);
            }
            i++;
         }
         var progressData:Object = this._skillsProgressArray[index];
         trace("fillSkillsList::progressData",progressData);
         var branchPoints:uint = this.getBranchPoints();
         this.setSkillProgress(branchPoints);
         this.handleAntiskills();
         this.checkButton();
      }
      
      public function lock(value:Boolean) : void
      {
         var newAlpha:Number = value ? 0.5 : 1;
         var itemsEnabled:Boolean = !value;
         this.btn_set.mouseEnabled = itemsEnabled;
         this.btn_set.alpha = newAlpha;
         this.btn_cancel.mouseEnabled = itemsEnabled;
         this.btn_cancel.alpha = newAlpha;
         this.btn_reroll.mouseEnabled = itemsEnabled;
         this.btn_reroll.alpha = newAlpha;
         for(var i:uint = 0; i < this._branches.length; i++)
         {
            var br:* = this._branches[i];
            br.mouseEnabled = itemsEnabled;
            br.alpha = newAlpha;
         }
      }
      
      public function setNameChangeAnswer(value:Boolean, message:String) : void
      {
         if(this._enterName)
         {
            if(value)
            {
               MainScene.stage.removeChild(this._enterName);
            }
            else
            {
               this._enterName.textField_error.text = message;
            }
         }
      }
      
      public function setName(value:String) : void
      {
         trace("CW:updateNickName setName",value);
      }
      
      public function showTitulList(event:MouseEvent) : void
      {
         var _avatarstitulList:Array;
         var _objectD:Object;
         var i:int;
         var _object:Object;
         if(!this.titulList.visible)
         {
            _avatarstitulList = new Array();
            _objectD = {name:"нет титула"};
            _avatarstitulList.push(_objectD);
            for(i = 0; i < 10; i++)
            {
               _object = {name:"Титул №" + i.toString()};
               _avatarstitulList.push(_object);
            }
            event.stopImmediatePropagation();
            this.titulList.addEventListener(MouseEvent.CLICK,this.onStageClickTitul);
            stage.addEventListener(MouseEvent.CLICK,this.onStageClickTitul);
            this.titulList.visible = true;
            this.titulList.list.dataProvider = new DataProvider(_avatarstitulList);
         }
         else
         {
            this.hideTituls();
         }
      }
      
      public function showEditName(event:ButtonEvent) : void
      {
         if(!this._enterName)
         {
            _enterName = new MovieClip();
         }
         MainScene.stage.addChild(this._enterName);
         MainScene.layout.reset();
      }
      
      public function apply_char_stats(value:Array) : void
      {
         trace("CW:apply_char_stats_len:",value.length);
         for(var i:uint = 0; i < value.length; i++)
         {
            for(var k:uint = 0; k < this._startStats.length; k++)
            {
               if(this._startStats[k].prop_id == value[i].prop_id)
               {
                  this._startStats[k].new_value = value[i].prop_value;
               }
            }
         }
      }
      
      protected function resetProps() : void
      {
         for(var k:uint = 0; k < this._startStats.length; k++)
         {
            this._startStats[k].new_value = this._startStats[k].prop_value;
         }
      }
      
      public function setPropsDescriptor(value:PropertiesDescriptor) : void
      {
         _propsDescriptor = value;
      }
      
      public function setCharacteristics(value:Array) : void
      {
         trace("setCharacteristics::",value ? value.length : "null");
         for(var k:uint = 0; k < value.length; k++)
         {
            var data:Object = value[k];
            var propDescription:Object = this._propsDescriptor.getPropertyDescription(data.prop_id);
            data.new_value = data.prop_value;
            if(propDescription.direction == PropertyDirection.HIGHER_IS_BETTER)
            {
               data.prefix = data.prop_value >= 0 ? "+" : "";
            }
            else
            {
               data.prefix = "-";
            }
            trace("setCharacteristics::iterate",data.prop_id,propDescription ? propDescription.direction : "nl",data.prop_value);
            data.name = propDescription.prop_name;
            data.direction = propDescription.direction;
            if(propDescription.postfix != undefined)
            {
               data.postfix = propDescription.postfix;
            }
            else
            {
               data.postfix = "";
            }
         }
         _startStats = value;
      }
      
      public function setAchievements(p:Array) : void
      {
      }
      
      public function setData(value:Object) : void
      {
         var branchPoints:uint;
         trace("CW:SetDataCharWindow");
         _data = value;
         if(value.points_available != undefined)
         {
            this.setPoints(value.points_available);
            branchPoints = this.getBranchPoints();
            this.setSkillProgress(branchPoints);
         }
         if(value.trees != undefined)
         {
            trace("CW:updateTress");
            points_aviable = value.points_available;
            this.updatePerkTrees(value.trees);
         }
         if(value.points_unlocked != undefined)
         {
         }
         if(value.perks != undefined)
         {
            this.updatePerks(value.perks);
         }
         if(value.experience_next_level != 0)
         {
            if(value.experience_delta != undefined)
            {
               if(value.experience_delta > value.experience_current)
               {
                  value.experience_delta = value.experience_current;
               }
            }
         }
         this.checkButton();
      }
      
      public function createPerkTree(value:Object) : void
      {
         this._treesData.push(value);
         this.createPerkTrees(this._treesData);
      }
      
      public function set_char_stats(value:Array) : void
      {
         this.setCharacteristics(value);
      }
      
      protected function updateProps() : void
      {
         var br:Array = [];
         for(var k:uint = 0; k < this._branches.length; k++)
         {
            var branch:* = this._branches[k];
            br.push({
               id:branch.getData().id,
               value:branch.selectedIndex
            });
         }
         ExternalInterface.call("on_skills_tree_selection_changed",br);
      }
      
      protected function onRenderProps(e:Event) : void
      {
         removeEventListener(Event.RENDER,this.onRenderProps);
         this.updateProps();
      }
      
      protected function onPerkUpdate(e:Event) : void
      {
         var branchOrigin:* = e.target;
         addEventListener(Event.RENDER,this.onRenderProps);
      }
      
      protected function createStats(value:Object) : void
      {
         var count:int = (value.opened);
         var item_data:Array = value.slots[0].properties;
         if(!item_data)
         {
            return;
         }
         for(var s:* in item_data)
         {
            var prop_value:Object = item_data[s];
            prop_value.value = 0;
            prop_value.new_value = 0;
            this._perkItems.push(prop_value);
         }
      }
      
      public function createPerkTrees(value:Array) : void
      {
         var _br_sy:int = 300;
         var _br_margin:int = 150;
         for(var i:int = 0; i < value.length; i++)
         {
            if(this._branches.length <= i || !this._branches[i])
            {
               trace("CW:creating new branch!");
               var br:* = new MovieClip();
               br.addEventListener(Event.CHANGE,this.onPerkUpdate);
               br.rotation = 90;
               this._branches.push(br);
               br.name = "perk_tress_" + String(i);
               br.index = i;
               this._branchesDict[value[i].id] = br;
               var ch:MovieClip = br.getChildByName("icon") as MovieClip;
               if(ch)
               {
                  ch.gotoAndStop(i + 1);
               }
               br.setData(value[i]);
               this.createStats(value[i]);
               addChild(br);
               br.unlock(0);
            }
            else
            {
               br = this._branches[i];
            }
            br.scaleX = 1.6;
            br.scaleY = 1.6;
            br.x = 610 + _br_margin * i;
            br.y = 130;
            addChild(br);
         }
         _charStats = new DataProvider(this._perkItems);
      }
      
      public function checkButton() : void
      {
         this.btn_set.enabled = false;
         this.btn_cancel.enabled = false;
         var skillsData:Object = this._skillsProgressArray[this._activeSkillBranch];
         if(skillsData)
         {
            trace("CW::checkButton");
            Util.logObject(skillsData);
            this.btn_reroll.enabled = skillsData.points > 0;
         }
         var l:uint = this._skillsArray.length;
         for(var i:uint = 0; i < l; i++)
         {
            var rendererData:Object = this._skillsArray[i];
            if(rendererData.value != rendererData.newValue)
            {
               this.btn_set.enabled = true;
               this.btn_cancel.enabled = true;
               break;
            }
         }
         if(!this._branches)
         {
            return;
         }
         for(var j:int = 0; j < this._branches.length; j++)
         {
            var br:* = this._branches[j];
            if((br.anyChanges()))
            {
               this.btn_set.enabled = true;
               this.btn_cancel.enabled = true;
               break;
            }
         }
         for(j = 0; j < this._branches.length; j++)
         {
            br = this._branches[j];
            if(br.getData().openedOrigin > 0)
            {
               this.btn_reroll.enabled = true;
               break;
            }
         }
      }
      
      override public function closeWindow(e:Event = null) : void
      {
         super.closeWindow(e);
         if(this.btn_cancel.enabled)
         {
            this.cancelChanges(null);
         }
      }
      
      protected function showAvatars(event:MouseEvent) : void
      {
         var _avatars:Array;
         var i:int;
         var icon:MovieClip;
         var _object:Object;
         if(!this.avatarList.visible)
         {
            _avatars = new Array();
            for(i = 0; i < 10; i++)
            {
               icon = new MovieClip();
               icon.gotoAndStop(i % 2 + 1);
               _object = {
                  name:"Нагибатель",
                  icon:icon
               };
               _avatars.push(_object);
            }
            event.stopImmediatePropagation();
            this.avatarList.addEventListener(MouseEvent.CLICK,this.onStageClick);
            stage.addEventListener(MouseEvent.CLICK,this.onStageClick);
            addChild(this.avatarList);
            this.avatarList.visible = true;
            this.avatarList.list.dataProvider = new DataProvider(_avatars);
         }
         else
         {
            this.hideAvatars();
         }
      }
      
      protected function onStageClick(e:MouseEvent) : void
      {
         if(!stage)
         {
            return;
         }
         if(stage.focus === this.avatarList)
         {
            return;
         }
         if(e.currentTarget == this.avatarList)
         {
            e.stopImmediatePropagation();
         }
         else
         {
            this.hideAvatars();
         }
      }
      
      protected function hideAvatars(event:MouseEvent = null) : void
      {
         this.avatarList.removeEventListener(MouseEvent.CLICK,this.onStageClick);
         stage.removeEventListener(MouseEvent.CLICK,this.onStageClick);
         this.avatarList.visible = false;
      }
      
      protected function hideTituls(event:MouseEvent = null) : void
      {
         this.titulList.removeEventListener(MouseEvent.CLICK,this.onStageClickTitul);
         stage.removeEventListener(MouseEvent.CLICK,this.onStageClickTitul);
         this.titulList.visible = false;
      }
      
      protected function setNewTitul(event:MouseEvent) : void
      {
         var _list:TileListSmoothR = this.titulList.list;
         var _render:* = _list.getRendererAt(_list.selectedIndex,_list.scrollPosition);
         this.hideTituls();
      }
      
      protected function setNewAvatar(event:MouseEvent) : void
      {
         var _list:TileListSmoothR = this.avatarList.list;
         var _render:* = _list.getRendererAt(_list.selectedIndex,_list.scrollPosition);
         this.hideAvatars();
      }
      
      protected function onStageClickTitul(e:MouseEvent) : void
      {
         if(!stage)
         {
            return;
         }
         if(stage.focus === this.titulList)
         {
            return;
         }
         if(e.currentTarget == this.titulList)
         {
            e.stopImmediatePropagation();
         }
         else
         {
            this.hideTituls();
         }
      }
      
      protected function scrollAchiv(event:ButtonEvent) : void
      {
      }
      
      protected function saveChanges(e:ButtonEvent = null) : void
      {
         trace("cleanUpSkills",this.newSkills.length);
         this.newSkills.length = 0;
         var l:uint = this._skillsArray.length;
         for(var i:int = 0; i < this._skillsArray.length; i++)
         {
            if(this._skillsArray[i].newValue > 0)
            {
               this.newSkills.push({
                  id:uint(this._skillsArray[i].id),
                  value:uint(this._skillsArray[i].newValue)
               });
            }
         }
         for(i = 0; i < this.newSkills.length; i++)
         {
            trace("Skills::saveChanges_for",this.newSkills[i].id,"new value::",this.newSkills[i].value);
         }
         ExternalInterface.call("apply_skills",this.newSkills);
         this.btn_set.enabled = false;
         this.btn_cancel.enabled = false;
         trace("changesSaved");
      }
      
      protected function rerollChanges(e:ButtonEvent) : void
      {
         var _obj:Object = new Object();
         _obj.price = this.reroll_cost;
         _obj.price_premium = this.reroll_cost_premium;
         _obj.text = LocalizationData.getName("message_reroll_text");
         _obj.branch = this._activeSkillBranch;
         var progressData:Object = this._skillsProgressArray[this._activeSkillBranch];
         if(progressData)
         {
            _obj.points = progressData.points;
         }
         else
         {
            _obj.points = 0;
         }
         Message.showMessage("reroll",_obj,true);
      }
      
      protected function cancelChanges(e:ButtonEvent) : void
      {
         var l:uint = this._skillsArray.length;
         for(var i:int = 0; i < this._skillsArray.length; i++)
         {
            this._skillsArray[i].newValue = this._skillsArray[i].value;
         }
         if(this._data)
         {
            this.setPoints(this._data.points_available);
         }
         this.fillSkillsList(this._activeSkillBranch);
         this.btn_set.enabled = false;
         this.btn_cancel.enabled = false;
      }
      
      protected function setPoints(value:int) : void
      {
         trace("setPoints",value);
         this.point_available_value.text = value.toString();
         points_aviable = value;
      }
      
      protected function updatePerks(value:Array) : void
      {
         this.btn_set.enabled = false;
         this.btn_cancel.enabled = false;
         for(var j:int = 0; j < this._branches.length; j++)
         {
            var br:* = this._branches[j];
            br.selectPerks(value);
         }
      }
      
      protected function updatePerkTrees(value:Array) : void
      {
         for(var j:int = 0; j < this._branches.length; j++)
         {
            var br:* = this._branches[j];
            br.unlock(0);
         }
         if(value.length == 0)
         {
            this.resetProps();
         }
         trace("CW:updatePerkTrees",value.length);
         for(var i:int = 0; i < value.length; i++)
         {
            br = this._branchesDict[value[i].id];
            br.unlock(value[i].points);
         }
      }
      
      protected function checkPoints() : void
      {
         var totalSelected:int = 0;
         for(var i:int = 0; i < this._branches.length; i++)
         {
            var br:* = this._branches[i];
            totalSelected += br.getDiff();
         }
         trace("checkPoints");
         this.setPoints(this._data.points_available - totalSelected);
      }
   }
}

