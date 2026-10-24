package survarium.controls
{
   import flash.display.DisplayObject;
   import flash.display.Stage;
   import flash.events.TimerEvent;
   import flash.utils.Timer;

   // The key that hides the interface (docs/notes/hide-ui-key.md, exe patch 62): every shown child
   // of the stage is hidden (the movie's root and every layer put on the stage: windows, popups,
   // messages, menus, tooltips, the dragged item, the loading screen), and a layer shown or added
   // while hidden is hidden too (checked every 100 ms), so nothing of the interface stays on the
   // screen. The movie goes on running. show() shows again exactly the layers it hid.
   public class InterfaceHider
   {
      private var _stage:Stage;

      private var _hidden:Array;

      private var _timer:Timer;

      public function InterfaceHider(stageRef:Stage)
      {
         super();
         _stage = stageRef;
         _timer = new Timer(100);
         this._timer.addEventListener(TimerEvent.TIMER,this.onTimer);
      }

      public function get hidden() : Boolean
      {
         return this._hidden != null;
      }

      public function hide() : void
      {
         if(this._hidden != null)
         {
            return;
         }
         _hidden = [];
         this.hideShown();
         this._timer.start();
      }

      // Shows what hide hid; true when it was hidden.
      public function show() : Boolean
      {
         if(this._hidden == null)
         {
            return false;
         }
         this._timer.stop();
         for each(var child:DisplayObject in this._hidden)
         {
            child.visible = true;
         }
         _hidden = null;
         return true;
      }

      private function hideShown() : void
      {
         var i:int = 0;
         while(i < this._stage.numChildren)
         {
            var child:DisplayObject = this._stage.getChildAt(i);
            if(child != null && child.visible)
            {
               child.visible = false;
               if(this._hidden.indexOf(child) < 0)
               {
                  this._hidden.push(child);
               }
            }
            i++;
         }
      }

      private function onTimer(e:TimerEvent) : void
      {
         if(this._hidden != null)
         {
            this.hideShown();
         }
      }
   }
}
