package survarium.buttons
{
   import com.greensock.TweenLite;
   import flash.display.MovieClip;
   import flash.text.TextField;
   
   public class ButtonWarning extends MovieClip
   {
      
      public var background:MovieClip;
      
      public var textFieldNotification:TextField;
      
      protected var borderOffset:uint = 6;
      
      protected var startSize:uint = 0;
      
      public function ButtonWarning()
      {
         super();
         startSize = this.background.width;
      }
      
      public function showWarning(message:String) : void
      {
         var targetWidth:Number;
         if(message)
         {
            alpha = 1;
            visible = true;
            this.textFieldNotification.text = message;
            targetWidth = this.textFieldNotification.textWidth + this.borderOffset * 2;
            this.textFieldNotification.width = 1;
            this.background.width = this.startSize;
            this.textFieldNotification.alpha = 1;
            this.background.alpha = 1;
            TweenLite.killTweensOf(this);
            TweenLite.killTweensOf(this.textFieldNotification);
            TweenLite.killTweensOf(this.background);
            TweenLite.to(this.textFieldNotification,1,{width:targetWidth});
            TweenLite.to(this.background,1,{
               width:targetWidth + this.textFieldNotification.x,
               onComplete:this.hideMe
            });
         }
         else
         {
            visible = false;
         }
      }
      
      protected function hideMe() : void
      {
         TweenLite.to(this,0.7,{
            delay:3,
            alpha:0
         });
      }
   }
}

