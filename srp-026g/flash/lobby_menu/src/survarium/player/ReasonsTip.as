package survarium.player
{
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.utils.ConstrainedElement;

   // The greyed Play's and Join's tip (presets.md, the reasons): the game's
   // tooltip (SrvProfileTipRendererStyled, as Autobuy's), its text a
   // line a reason, with the left margin as wide as the right (the owner's
   // of 2026-10-23: "a bit more padding from the left"). The stock renderer
   // pads the right by 5 more than its margins and takes one line only.
   public class ReasonsTip extends ToolTip
   {

      public function ReasonsTip()
      {
         super();
      }

      override protected function showTip(e:Event = null) : void
      {
         super.showTip(e);
         try
         {
            this.fit(_tip_content);
         }
         catch(err:Error)
         {
         }
      }

      // The renderer's margins (its Constraints element, the stock layout's
      // own setting): the left one set so the text's gap on the left is the
      // one on the right (the renderer adds 5 on the right, the field's
      // gutter is 2 a side); every line shown; its size as the renderer
      // works it out.
      private function fit(renderer:Object) : void
      {
         if(renderer == null || renderer.textField == null || renderer.constraints == null)
         {
            return;
         }
         var field:TextField = renderer.textField as TextField;
         var element:ConstrainedElement = renderer.constraints.getElement("textField");
         if(field == null || element == null)
         {
            return;
         }
         element.left = Math.max(element.left,element.right + 1);
         field.multiline = true;
         field.wordWrap = false;
         field.text = String(tipData != null ? tipData.name : "");
         var w:Number = Math.ceil(field.textWidth + element.left + element.right + 5);
         var h:Number = Math.ceil(field.textHeight + element.top + element.bottom + 5);
         renderer.width = w;
         renderer.height = h;
         renderer.constraints.update(w,h);
      }
   }
}
