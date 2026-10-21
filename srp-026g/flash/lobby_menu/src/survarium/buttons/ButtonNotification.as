package survarium.buttons
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.constants.ImportantMessageType;
   
   public class ButtonNotification extends ListItemRenderer
   {
      
      public var icon:MovieClip;
      
      public var background:MovieClip;
      
      public var emptyBack:MovieClip;
      
      public var textFieldNotification:TextField;
      
      public var iconType:MovieClip;
      
      protected var borderOffset:uint = 6;
      
      public function ButtonNotification()
      {
         super();
      }
      
      override protected function updateText() : void
      {
         super.updateText();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
      
      override public function get data() : Object
      {
         return super.data;
      }
      
      override public function set data(value:Object) : void
      {
         var textWidth:Number;
         super.data = value;
         if(_data)
         {
            trace("NB::",_data.type);
            visible = true;
            this.textFieldNotification.text = _data.label;
            textWidth = this.textFieldNotification.textWidth + 4;
            this.emptyBack.width = textWidth + this.borderOffset;
            if(!_data.created)
            {
               this.iconType.gotoAndStop(_data.type == ImportantMessageType.SQUAD_INVITE ? 1 : 2);
               this.icon.gotoAndPlay(2);
               this.textFieldNotification.width = 1;
               _data.created = true;
               this.background.width = 4;
               TweenLite.to(this.textFieldNotification,0.2,{
                  delay:0.2,
                  width:textWidth
               });
               TweenLite.to(this.background,0.2,{
                  delay:0.2,
                  width:textWidth + this.borderOffset
               });
            }
            else
            {
               this.iconType.gotoAndStop(1);
               this.icon.gotoAndStop(1);
               this.textFieldNotification.width = textWidth;
               this.background.width = textWidth + this.borderOffset;
            }
         }
         else
         {
            visible = false;
         }
      }
   }
}

