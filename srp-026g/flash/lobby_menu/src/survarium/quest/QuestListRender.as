package survarium.quest
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.TimerEvent;
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.Util;
   import survarium.controls.QuestWindow;
   
   public class QuestListRender extends ListItemRenderer
   {
      
      public var questIcon:MovieClip;
      
      public var textFieldTime:TextField;
      
      protected var _start_time:int = 0;
      
      public function QuestListRender()
      {
         super();
         MainScene.getTimerInstance().addEventListener(TimerEvent.TIMER,this.timerStep,false,0,true);
         _tooltipRendererName = "SrvProfileTipRenderer";
      }
      
      protected function timerStep(event:Event = null) : void
      {
         invalidateData();
      }
      
      override public function setData(newData:Object) : void
      {
         this.data = newData;
         if(_data)
         {
            _start_time = _data.start_time;
         }
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         constraintsDisabled = true;
         this.textFieldTime.defaultTextFormat = this.textFieldTime.getTextFormat();
      }
      
      override protected function updateText() : void
      {
         var timeLeft:int;
         if(!data)
         {
            this.questIcon.visible = false;
            this.textFieldTime.text = "";
            return;
         }
         _data.name = _data.quest_label;
         if(enabled && _data && _data.time_limit)
         {
            if(this.textFieldTime)
            {
               timeLeft = Math.max(0,_data.time_limit - (QuestWindow.getTime() - this._start_time));
               this.textFieldTime.text = Util.getTimeString(timeLeft);
            }
         }
         this.questIcon.visible = true;
         this.questIcon.gotoAndStop(_data.quest_icon + 1);
      }
   }
}

