package survarium.buttons
{
   import flash.display.BitmapData;
   import flash.display.Graphics;
   import flash.display.MovieClip;
   import flash.events.MouseEvent;
   import flash.geom.Matrix;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ListItemRenderer;
   import scaleform.clik.events.InputEvent;
   import survarium.Util;
   import survarium.data.LocalizationData;
   
   public class DailyDropRenderer extends ListItemRenderer
   {
      
      public var droppedItem:MovieClip;
      
      public var progressIndicator:MovieClip;
      
      public var progressIndicatorFull:MovieClip;
      
      public var emptyBonusIcon:MovieClip;
      
      public var completedButton:MovieClip;
      
      public var textLabel:TextField;
      
      protected var backTexture:BitmapData = new BlueTexture();
      
      public function DailyDropRenderer()
      {
         super();
         stop();
         trace("Contruct::DailyDropRenderer");
      }
      
      override public function set data(param1:Object) : void
      {
         super.data = param1;
         if(param1)
         {
            selected = param1.selected;
         }
         invalidateData();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
         tooltipRendererName = "SrvProfileTipRendererStyled";
         mouseChildren = false;
         this.progressIndicator.gotoAndStop(3);
      }
      
      override protected function handleMouseRelease(param1:MouseEvent) : void
      {
      }
      
      override protected function handleMousePress(param1:MouseEvent) : void
      {
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
      }
      
      protected function drawBackGround(param1:Graphics) : void
      {
         var _loc2_:Matrix = new Matrix();
         _loc2_.translate(-_data.index * 76,0);
         param1.clear();
         param1.beginBitmapFill(this.backTexture,_loc2_,true,true);
         param1.drawRect(0,0,76,76);
         param1.endFill();
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateData();
         }
      }
      
      protected function updateData() : void
      {
         trace("DDR::updateText",name,_data);
         if(_data)
         {
            _data.name = LocalizationData.getName("daily_bonus_progress") + "\n" + Util.styledChar(_data.progress,"white") + "/" + Util.styledChar(_data.progressRequired,"gold") + " " + LocalizationData.getName("daily_bonus_victories");
            this.textLabel.textColor = _data.last ? 15317048 : (_selected ? 16704178 : 15000804);
            this.textLabel.alpha = _selected ? 1 : 0.7;
            this.textLabel.text = _data.dayLabel;
            this.progressIndicator.gotoAndStop(3);
            this.progressIndicator.alpha = 0.1;
            this.drawBackGround(this.progressIndicator.container.graphics);
            if(_data.progress > 0)
            {
               this.progressIndicatorFull.visible = true;
               this.progressIndicatorFull.gotoAndStop(_data.progress);
               this.drawBackGround(this.progressIndicatorFull.container.graphics);
            }
            else
            {
               this.progressIndicatorFull.gotoAndStop(1);
               this.progressIndicatorFull.visible = false;
            }
            if(this.completedButton)
            {
               this.completedButton.visible = _data.completed;
            }
            trace("updatedDDR::",_data.dayLabel,_data.itemData);
         }
      }
   }
}

