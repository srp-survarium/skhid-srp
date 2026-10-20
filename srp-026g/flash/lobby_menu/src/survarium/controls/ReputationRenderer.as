package survarium.controls
{
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.Util;
   
   public class ReputationRenderer extends ListItemRenderer
   {
      
      public var factionProgress:MovieClip;
      
      public var factionIcon:MovieClip;
      
      public var lockIcon:MovieClip;
      
      public var hintButton:Button;
      
      public var hintButton2:Button;
      
      public var textFieldPercent:TextField;
      
      public var textFieldBonus:TextField;
      
      private var _unlocked:Boolean = false;
      
      public function ReputationRenderer()
      {
         super();
      }
      
      override public function set data(param1:Object) : void
      {
         _data = param1;
         if(_data)
         {
            _unlocked = _data.faction_enabled;
            mouseEnabled = true;
         }
         invalidateData();
      }
      
      public function showSelection(param1:Boolean) : void
      {
         selected = param1;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         mouseChildren = true;
         this.factionProgress.mouseEnabled = false;
         this.hintButton.alpha = 0;
         this.hintButton.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.hintButton2.alpha = 0;
         this.hintButton2.tooltipRendererName = "SrvProfileTipRendererStyled";
         this.lockIcon.mouseEnabled = false;
      }
      
      override protected function updateText() : void
      {
         trace("RR:updateText::",name);
         mouseChildren = true;
         this.hintButton.mouseEnabled = enabled;
         this.hintButton.visible = true;
         if(_data)
         {
            this.hintButton.data = {name:Util.styledChar(_data.faction_name,"white") + "\n" + _data.faction_label + Util.styledChar("fff\n\n\n","small") + _data.faction_hint};
            this.factionProgress.visible = true;
            if(_data.faction_progress == 0)
            {
               _data.faction_progress = 1;
            }
            this.factionProgress.visible = _data.faction_progress != 0;
            this.factionProgress.set_progress(_data.faction_progress);
            if(_data.selected)
            {
            }
            this.factionIcon.gotoAndStop(_data.faction_id);
            if(_data.faction_bonus != undefined && _data.faction_bonus != "" && false)
            {
               this.textFieldPercent.mouseEnabled = this.textFieldBonus.mouseEnabled = false;
               this.textFieldPercent.visible = true;
               this.textFieldBonus.visible = true;
               this.textFieldBonus.text = String(_data.faction_bonus);
            }
            else
            {
               this.textFieldPercent.visible = false;
               this.textFieldBonus.visible = false;
            }
         }
         else
         {
            this.hintButton2.data = null;
            this.hintButton.data = null;
            this.factionProgress.visible = false;
            this.textFieldPercent.visible = false;
            this.textFieldBonus.visible = false;
         }
         this.hintButton2.mouseEnabled = this.hintButton2.visible = false;
         this.factionIcon.alpha = this._unlocked ? 1 : 0.6;
         this.factionProgress.visible = this._unlocked;
         this.lockIcon.visible = !this._unlocked;
      }
   }
}

