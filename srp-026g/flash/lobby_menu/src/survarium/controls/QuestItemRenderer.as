package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import flash.text.TextFieldAutoSize;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.data.DataProvider;
   import survarium.Util;
   import survarium.data.LocalizationData;
   import survarium.constants.QuestStatus;
   
   public class QuestItemRenderer extends ListItemRenderer
   {
      
      public var labelTextField:TextField;
      
      public var rewardTextField:TextField;
      
      public var rewardBar:ButtonBar;
      
      public var taskList:ButtonBar;
      
      public var timeTextField:TextField;
      
      public var questIcon:MovieClip;
      
      public var background:MovieClip;
      
      public var rewardsBack:MovieClip;
      
      public var rewardField:TextField;
      
      public var idTextField:TextField;
      
      public var questStatusContainer:MovieClip;
      
      public var factionIcon:IconRenderer;
      
      public var rewardsHint:ToolTip;
      
      protected var _start_time:int = 0;
      
      protected var _scrollContent:ScrollContent;
      
      protected var _inProgress:Boolean = false;
      
      protected var _bottomShift:uint = 33;
      
      public function QuestItemRenderer()
      {
         super();
      }
      
      override public function get height() : Number
      {
         return _data && this.background ? this.background.height : 0;
      }
      
      override public function set data(value:Object) : void
      {
         _data = value;
         if(_data)
         {
            _start_time = value.start_time;
         }
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         constraintsDisabled = true;
         mouseChildren = true;
         this.rewardField.mouseEnabled = this.rewardBar.mouseEnabled = this.idTextField.mouseEnabled = this.taskList.mouseEnabled = this.questIcon.mouseEnabled = this.labelTextField.mouseEnabled = textField.mouseEnabled = false;
         this.rewardBar.mouseChildren = this.taskList.mouseChildren = this.questIcon.mouseChildren = false;
         this.taskList.setMaxFit(true);
         if(this.rewardsBack)
         {
            this.rewardsBack.mouseEnabled = false;
            this.rewardsBack.mouseChildren = false;
         }
         if(this.background)
         {
            this.background.mouseEnabled = false;
         }
         if(this.timeTextField)
         {
            this.timeTextField.mouseEnabled = false;
            _inProgress = true;
         }
         this.rewardsHint.tooltipRendererName = "SrvProfileTipRenderer";
         textField.autoSize = TextFieldAutoSize.LEFT;
         if(this._inProgress)
         {
            MainScene.getTimerInstance().addEventListener(TimerEvent.TIMER,this.timerStep,false,0,true);
         }
         if(!this._inProgress)
         {
            _scrollContent = new ScrollContent();
            this._scrollContent.x = int(textField.x);
            this._scrollContent.y = int(this.labelTextField.y);
            addChild(this._scrollContent);
            this.labelTextField.x -= this._scrollContent.x;
            this.labelTextField.y = 0;
            textField.x -= this._scrollContent.x;
            textField.y = int(this.labelTextField.height - 3);
            this.factionIcon.x -= this._scrollContent.x;
            this.factionIcon.y = 4;
            this.taskList.x -= this._scrollContent.x;
            this._scrollContent.width = this.timeTextField ? 437 : 437;
            this._scrollContent.height = 139;
            this._scrollContent.addContent(this.labelTextField);
            this._scrollContent.addContent(textField);
            this._scrollContent.addContent(this.taskList);
            this._scrollContent.addContent(this.factionIcon);
         }
      }
      
      protected function timerStep(event:Event = null) : void
      {
         var timeLeft:int;
         if(enabled && _data && _data.time_limit)
         {
            if(this.timeTextField)
            {
               timeLeft = Math.max(0,_data.time_limit - (QuestWindow.getTime() - this._start_time));
               this.timeTextField.text = Util.getTimeString(timeLeft);
            }
         }
      }
      
      override protected function draw() : void
      {
         super.draw();
         mouseChildren = true;
      }
      
      override public function toString() : String
      {
         return super.toString() + " children:" + mouseChildren.toString() + " rewardsHint:" + this.rewardsHint.mouseEnabled.toString();
      }
      
      override protected function updateText() : void
      {
         var debugId:String;
         var rewards:Array;
         var i:uint;
         mouseChildren = true;
         if(_data)
         {
            visible = true;
            this.rewardBar.spacing = 20;
            this.taskList.height = 80;
            this.labelTextField.text = _data.quest_label;
            textField.text = _data.text;
            this.rewardField.text = LocalizationData.getName("reward");
            if(this._inProgress)
            {
               this.timerStep();
            }
            this.taskList.y = int(textField.y + textField.textHeight + 10);
            if(!this.rewardsHint.tipData)
            {
               this.rewardsHint.tipData = {name:LocalizationData.getName("quest_reward_info")};
            }
            this.taskList.dataProvider = new DataProvider(_data.tasks);
            this.taskList.validateNow();
            this.taskList.height = this.taskList.container.height * this.taskList.scaleY + 10;
            if(this.factionIcon)
            {
               this.factionIcon.content = _data.faction_icon ? _data.faction_icon.clone() : null;
            }
            trace("updateQuest::",_data.tasks,this.taskList.height,_data.faction_icon,this,this.factionIcon.parent,this.factionIcon.alpha,this.factionIcon.visible,this.factionIcon.numChildren);
            debugId = MainScene.showDebugInfo ? _data.dict_id : "";
            if(this.idTextField && this.idTextField.text != debugId)
            {
               this.idTextField.text = debugId;
            }
            this.questIcon.gotoAndStop(_data.quest_icon + 1);
            if(_data.reward)
            {
               this.rewardBar.x = this.rewardField.x + this.rewardField.textWidth + 17;
               rewards = _data.reward as Array;
               if(rewards)
               {
                  i = 0;
                  while(i < rewards.length)
                  {
                     rewards[i].useMultiplier = _data.useMultiplier != undefined ? _data.useMultiplier : true;
                     i++;
                  }
               }
               this.rewardBar.dataProvider = new DataProvider(_data.reward);
               this.rewardBar.validateNow();
            }
            if(!this._inProgress)
            {
               if(this.questStatusContainer)
               {
                  if(_data.status != undefined)
                  {
                     this.questStatusContainer.visible = true;
                     this.questStatusContainer.textField.alpha = 0.8;
                     this.questStatusContainer.textField.text = LocalizationData.getName(String("quest_status_" + _data.status));
                     if(_data.status == QuestStatus.FAILED)
                     {
                        this.rewardField.visible = this.rewardsHint.visible = this.rewardBar.visible = false;
                     }
                     else
                     {
                        this.rewardField.visible = this.rewardsHint.visible = this.rewardBar.visible = true;
                     }
                  }
                  else
                  {
                     this.questStatusContainer.visible = false;
                  }
               }
               if(this._scrollContent)
               {
                  this._scrollContent.invalidateData();
                  this._scrollContent.validateNow();
               }
            }
            else
            {
               this.rewardBar.y = this.taskList.y + this.taskList.height + 22;
               this.rewardsHint.y = this.rewardBar.y + 3;
               this.rewardField.y = this.rewardBar.y - 1;
               this.background.height = this.rewardBar.y + this._bottomShift;
               this.idTextField.y = this.background.height - this.idTextField.height - 5;
            }
         }
         else
         {
            visible = false;
            if(this.factionIcon)
            {
               this.factionIcon.content = null;
            }
         }
      }
   }
}

