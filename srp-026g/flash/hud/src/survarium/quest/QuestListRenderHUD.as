package survarium.quest
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.controls.SectionIndicator;
   
   public class QuestListRenderHUD extends ListItemRenderer
   {
      
      public var questIcon:MovieClip;
      
      public var questProgressLine:Sprite;
      
      public var sectionIndicator:SectionIndicator;
      
      protected var _start_time:int = 0;
      
      public function QuestListRenderHUD()
      {
         super();
         this.sectionIndicator.renderer = "SectionRenderer";
      }
      
      override public function setData(data:Object) : void
      {
         this.data = data;
         invalidateData();
      }
      
      override public function get width() : Number
      {
         return 255;
      }
      
      override public function get height() : Number
      {
         return 187;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         constraintsDisabled = true;
         textField.defaultTextFormat = textField.getTextFormat();
      }
      
      override protected function updateText() : void
      {
         if(!data)
         {
            this.questIcon.visible = false;
            textField.defaultTextFormat = textField.getTextFormat();
            textField.text = " ";
            return;
         }
         var isRatio:Boolean = _data.is_ratio;
         var currentProgress:Number = Math.floor((_data.current_progress + 0.005) / 0.1) * 0.1;
         var required:Number = Math.floor((_data.required + 0.005) / 0.1) * 0.1;
         var progressLabel:String;
         trace("questIcon::",currentProgress,required,_data.current_progress,_data.required);
         if(int(currentProgress) != currentProgress)
         {
            progressLabel = currentProgress.toFixed(1);
         }
         else
         {
            progressLabel = currentProgress.toString();
         }
         textField.defaultTextFormat = textField.getTextFormat();
         if(!isRatio || required < 10)
         {
            textField.text = progressLabel;
         }
         else
         {
            textField.text = progressLabel + "%";
         }
         this.questIcon.visible = true;
         this.questIcon.gotoAndStop(_data.quest_icon + 1);
         this.sectionIndicator.useSections = !isRatio;
         this.sectionIndicator.minimum = 0;
         this.sectionIndicator.maximum = required;
         this.sectionIndicator.value = currentProgress;
      }
   }
}

